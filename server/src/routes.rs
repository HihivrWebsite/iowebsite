use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    normalize::normalize,
    state::{AppState, CacheDecision, CacheKey, UPSTREAM_BUDGET},
    APP_VERSION,
};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct HealthData {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_sec: u64,
}

#[derive(Debug, Serialize)]
pub struct VersionData {
    pub version: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct LiveStatusQuery {
    pub uid: Option<String>,
    pub room_id: Option<String>,
}

type ApiError = (StatusCode, Json<ApiResponse<()>>);

pub async fn health(State(state): State<AppState>) -> Json<ApiResponse<HealthData>> {
    Json(ApiResponse {
        code: 0,
        message: "ok".to_owned(),
        data: HealthData {
            status: "ok",
            version: APP_VERSION,
            uptime_sec: state.uptime().as_secs(),
        },
    })
}

pub async fn version() -> Json<ApiResponse<VersionData>> {
    Json(ApiResponse {
        code: 0,
        message: "ok".to_owned(),
        data: VersionData {
            version: APP_VERSION,
        },
    })
}

pub async fn live_status(
    State(state): State<AppState>,
    Query(query): Query<LiveStatusQuery>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let uid = parse_positive_id(query.uid.as_deref(), "uid")?;
    let room_id = parse_positive_id(query.room_id.as_deref(), "room_id")?;
    let key = CacheKey { uid, room_id };

    match state.decision(&key).await {
        CacheDecision::Fresh(data) => return Ok(cached_response(data, "ok")),
        CacheDecision::Snapshot(data) => return Ok(cached_response(data, "snapshot")),
        CacheDecision::Backoff => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiResponse {
                    code: 503,
                    message: "upstream retry backoff is active".to_owned(),
                    data: (),
                }),
            ));
        }
        CacheDecision::Ready => {}
    }

    let update_lock = state.update_lock(&key).await;
    let guard = update_lock.lock().await;

    // 持锁后重新检查：并发请求只会有一个真正访问 B 站。
    let decision = state.decision(&key).await;
    let data = match decision {
        CacheDecision::Fresh(data) => cached_response(data, "ok"),
        CacheDecision::Snapshot(data) => cached_response(data, "snapshot"),
        CacheDecision::Backoff => {
            drop(guard);
            state.release_update_lock(&key, update_lock).await;
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiResponse {
                    code: 503,
                    message: "upstream retry backoff is active".to_owned(),
                    data: (),
                }),
            ));
        }
        CacheDecision::Ready => {
            let fetched =
                tokio::time::timeout(UPSTREAM_BUDGET, state.upstream.fetch_sources(uid, room_id))
                    .await;

            let normalized = match fetched {
                Ok(Ok(sources)) => normalize(uid, room_id, &sources, Utc::now())
                    .map_err(|error| {
                        tracing::warn!(uid, room_id, error = %error, "normalization failed");
                        error.to_string()
                    })
                    .and_then(|value| {
                        serde_json::to_value(value).map_err(|error| {
                            tracing::warn!(uid, room_id, error = %error, "serialization failed");
                            error.to_string()
                        })
                    }),
                Ok(Err(error)) => {
                    tracing::warn!(uid, room_id, error = %error, "Bilibili upstream failed");
                    Err(error.to_string())
                }
                Err(_) => {
                    tracing::warn!(uid, room_id, "Bilibili upstream exceeded 5s budget");
                    Err("upstream request exceeded 5 seconds".to_owned())
                }
            };

            let result = match normalized {
                Ok(payload) => {
                    state.record_success(&key, payload.clone()).await;
                    Json(ApiResponse {
                        code: 0,
                        message: "ok".to_owned(),
                        data: payload,
                    })
                }
                Err(error) => {
                    let delay = state.record_failure(&key).await;
                    if let CacheDecision::Snapshot(snapshot) = state.decision(&key).await {
                        tracing::warn!(
                            uid,
                            room_id,
                            retry_delay_ms = delay.as_millis() as u64,
                            "upstream failed; serving last snapshot"
                        );
                        cached_response(snapshot, "snapshot")
                    } else {
                        tracing::warn!(
                            uid,
                            room_id,
                            retry_delay_ms = delay.as_millis() as u64,
                            "upstream failed and no snapshot exists"
                        );
                        return Err((
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(ApiResponse {
                                code: 503,
                                message: format!("Bilibili upstream unavailable: {error}"),
                                data: (),
                            }),
                        ));
                    }
                }
            };
            result
        }
    };

    drop(guard);
    state.release_update_lock(&key, update_lock).await;
    Ok(data)
}

fn parse_positive_id(value: Option<&str>, field: &'static str) -> Result<i64, ApiError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| bad_request(format!("{field} is required")))?;

    let parsed = value
        .parse::<i64>()
        .map_err(|_| bad_request(format!("{field} must be a positive integer")))?;

    if parsed <= 0 {
        return Err(bad_request(format!("{field} must be a positive integer")));
    }

    Ok(parsed)
}

fn bad_request(message: impl Into<String>) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse {
            code: 400,
            message: message.into(),
            data: (),
        }),
    )
}

fn cached_response(data: Value, message: &str) -> Json<ApiResponse<Value>> {
    Json(ApiResponse {
        code: 0,
        message: message.to_owned(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positive_ids_and_rejects_invalid_values() {
        assert_eq!(
            parse_positive_id(Some("85996232"), "uid").unwrap(),
            85996232
        );
        assert_eq!(
            parse_positive_id(Some(" 1890510937 "), "room_id").unwrap(),
            1890510937
        );
        assert!(parse_positive_id(None, "uid").is_err());
        assert!(parse_positive_id(Some("0"), "uid").is_err());
        assert!(parse_positive_id(Some("-1"), "uid").is_err());
        assert!(parse_positive_id(Some("abc"), "uid").is_err());
    }
}
