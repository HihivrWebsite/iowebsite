use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::Value;
use tokio::sync::{Mutex, RwLock};

use crate::upstream::UpstreamClient;

pub const CACHE_TTL: Duration = Duration::from_secs(20);
pub const UPSTREAM_BUDGET: Duration = Duration::from_secs(5);
const MAX_CACHE_ENTRIES: usize = 128;
const MAX_FAILURE_ENTRIES: usize = 128;
const MAX_BACKOFF_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub uid: i64,
    pub room_id: i64,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    data: Value,
    stored_at: Instant,
}

#[derive(Debug, Clone)]
struct FailureEntry {
    attempts: u32,
    retry_at: Instant,
}

#[derive(Debug, Clone)]
pub enum CacheDecision {
    Fresh(Value),
    Snapshot(Value),
    Backoff,
    Ready,
}

#[derive(Clone)]
pub struct AppState {
    pub upstream: UpstreamClient,
    cache: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    failures: Arc<RwLock<HashMap<CacheKey, FailureEntry>>>,
    update_locks: Arc<RwLock<HashMap<CacheKey, Arc<Mutex<()>>>>>,
    started_at: Instant,
}

impl AppState {
    pub fn new(upstream: UpstreamClient) -> Self {
        Self {
            upstream,
            cache: Arc::new(RwLock::new(HashMap::new())),
            failures: Arc::new(RwLock::new(HashMap::new())),
            update_locks: Arc::new(RwLock::new(HashMap::new())),
            started_at: Instant::now(),
        }
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub async fn decision(&self, key: &CacheKey) -> CacheDecision {
        let cache = self.cache.read().await;
        let entry = cache.get(key);

        if let Some(entry) = entry {
            if entry.stored_at.elapsed() <= CACHE_TTL {
                return CacheDecision::Fresh(entry.data.clone());
            }
        }

        let snapshot = entry.map(|entry| entry.data.clone());
        drop(cache);

        let failures = self.failures.read().await;
        let retry_active = failures
            .get(key)
            .is_some_and(|failure| failure.retry_at > Instant::now());
        drop(failures);

        if retry_active {
            if let Some(snapshot) = snapshot {
                return CacheDecision::Snapshot(snapshot);
            }
            return CacheDecision::Backoff;
        }

        CacheDecision::Ready
    }

    pub async fn update_lock(&self, key: &CacheKey) -> Arc<Mutex<()>> {
        let mut locks = self.update_locks.write().await;
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub async fn release_update_lock(&self, key: &CacheKey, lock: Arc<Mutex<()>>) {
        drop(lock);
        let mut locks = self.update_locks.write().await;
        if locks
            .get(key)
            .is_some_and(|candidate| Arc::strong_count(candidate) == 1)
        {
            locks.remove(key);
        }
    }

    pub async fn record_success(&self, key: &CacheKey, data: Value) {
        let mut cache = self.cache.write().await;
        evict_if_needed(&mut cache, MAX_CACHE_ENTRIES, |entry| entry.stored_at);
        cache.insert(
            key.clone(),
            CacheEntry {
                data,
                stored_at: Instant::now(),
            },
        );
        drop(cache);

        let mut failures = self.failures.write().await;
        failures.remove(key);
    }

    pub async fn record_failure(&self, key: &CacheKey) -> Duration {
        let now = Instant::now();
        let mut failures = self.failures.write().await;
        let attempts = failures
            .get(key)
            .map(|failure| failure.attempts.saturating_add(1))
            .unwrap_or(1);
        let delay = backoff_delay(attempts);

        evict_if_needed(&mut failures, MAX_FAILURE_ENTRIES, |entry| entry.retry_at);
        failures.insert(
            key.clone(),
            FailureEntry {
                attempts,
                retry_at: now + delay,
            },
        );

        delay
    }

    pub async fn cache_len(&self) -> usize {
        self.cache.read().await.len()
    }
}

fn backoff_delay(attempts: u32) -> Duration {
    if attempts == 0 {
        return Duration::ZERO;
    }

    let exponent = attempts.saturating_sub(1).min(5);
    let seconds = if exponent >= 5 {
        MAX_BACKOFF_SECONDS
    } else {
        1_u64 << exponent
    };
    Duration::from_secs(seconds)
}

fn evict_if_needed<T, F>(map: &mut HashMap<CacheKey, T>, limit: usize, timestamp: F)
where
    F: Fn(&T) -> Instant,
{
    if map.len() < limit {
        return;
    }

    let Some(oldest_key) = map
        .iter()
        .min_by_key(|(_, value)| timestamp(value))
        .map(|(key, _)| key.clone())
    else {
        return;
    };
    map.remove(&oldest_key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_and_capped() {
        assert_eq!(backoff_delay(1), Duration::from_secs(1));
        assert_eq!(backoff_delay(2), Duration::from_secs(2));
        assert_eq!(backoff_delay(3), Duration::from_secs(4));
        assert_eq!(backoff_delay(4), Duration::from_secs(8));
        assert_eq!(backoff_delay(5), Duration::from_secs(16));
        assert_eq!(backoff_delay(6), Duration::from_secs(30));
        assert_eq!(backoff_delay(100), Duration::from_secs(30));
    }
}

#[cfg(test)]
mod cache_tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn fresh_cache_and_failure_snapshot_work_during_backoff() {
        let state = AppState::new(UpstreamClient::new().expect("upstream client should build"));
        let key = CacheKey {
            uid: 85996232,
            room_id: 1890510937,
        };
        let payload = json!({"uid": 85996232, "live_status": 0});

        state.record_success(&key, payload.clone()).await;
        assert!(matches!(
            state.decision(&key).await,
            CacheDecision::Fresh(value) if value == payload
        ));

        // 测试内直接老化缓存，避免真实等待 20 秒。
        {
            let mut cache = state.cache.write().await;
            let entry = cache.get_mut(&key).expect("cached entry");
            entry.stored_at = Instant::now() - CACHE_TTL - Duration::from_secs(1);
        }

        state.record_failure(&key).await;
        assert!(matches!(
            state.decision(&key).await,
            CacheDecision::Snapshot(value) if value == payload
        ));
    }

    #[tokio::test]
    async fn first_failure_without_snapshot_enters_backoff() {
        let state = AppState::new(UpstreamClient::new().expect("upstream client should build"));
        let key = CacheKey { uid: 1, room_id: 2 };

        let delay = state.record_failure(&key).await;
        assert_eq!(delay, Duration::from_secs(1));
        assert!(matches!(state.decision(&key).await, CacheDecision::Backoff));
    }
}
