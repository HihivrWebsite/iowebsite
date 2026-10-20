use std::fmt;

use chrono::{DateTime, FixedOffset, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::upstream::UpstreamSources;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveStatusData {
    pub uid: i64,
    pub room_id: i64,
    pub uname: String,
    pub title: String,
    pub live_status: i8,
    pub live_time: Option<i64>,
    pub duration_sec: i64,
    pub online: i64,
    pub attention: i64,
    pub area_name: String,
    pub parent_area_name: String,
    pub cover: String,
    pub keyframe: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationError(String);

impl fmt::Display for NormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NormalizationError {}

pub fn normalize(
    requested_uid: i64,
    requested_room_id: i64,
    sources: &UpstreamSources,
    now: DateTime<Utc>,
) -> Result<LiveStatusData, NormalizationError> {
    let status_data = data_object(&sources.status, "get_status_info_by_uids")?;
    let init_data = data_object(&sources.init, "room_init")?;
    let info_data = data_object(&sources.info, "get_info")?;

    let status = keyed_entry(status_data, requested_uid, "get_status_info_by_uids")?;
    let status_object = required_object(status, "get_status_info_by_uids status")?;
    // data_object 已经确认 room_init/get_info 的 data 是对象，因此这里直接使用其字段 Map。
    let init_object = init_data;
    let info_object = info_data;

    let live_status = [
        first_number(init_object, &["live_status"]),
        first_number(status_object, &["live_status"]),
        first_number(info_object, &["live_status"]),
    ]
    .into_iter()
    .flatten()
    .find_map(normalize_live_status)
    .ok_or_else(|| {
        NormalizationError(
            "upstream returned no supported live_status (expected 0, 1 or 2)".to_owned(),
        )
    })?;

    let now_timestamp = now.timestamp();
    let live_time = if live_status == 1 {
        [
            status_object.get("live_time"),
            init_object.get("live_time"),
            info_object.get("live_time"),
        ]
        .into_iter()
        .flatten()
        .find_map(|value| parse_live_time(value, now_timestamp))
    } else {
        None
    };

    let duration_sec = match live_time {
        Some(timestamp) if timestamp <= now_timestamp => now_timestamp.saturating_sub(timestamp),
        _ => 0,
    };

    let uid = first_number(init_object, &["uid"])
        .or_else(|| first_number(info_object, &["uid"]))
        .or_else(|| first_number(status_object, &["uid"]))
        .unwrap_or(requested_uid)
        .max(0);
    let room_id = first_number(init_object, &["room_id"])
        .or_else(|| first_number(info_object, &["room_id"]))
        .or_else(|| first_number(status_object, &["room_id"]))
        .unwrap_or(requested_room_id)
        .max(0);

    let online = first_number(status_object, &["online"])
        .or_else(|| first_number(info_object, &["online"]))
        .unwrap_or(0)
        .max(0);
    let attention = first_number(info_object, &["attention"])
        .or_else(|| first_number(status_object, &["attention"]))
        .unwrap_or(0)
        .max(0);

    Ok(LiveStatusData {
        uid,
        room_id,
        uname: first_string(status_object, &["uname"])
            .or_else(|| first_string(info_object, &["uname"]))
            .unwrap_or_default()
            .to_owned(),
        title: first_string(status_object, &["title"])
            .or_else(|| first_string(info_object, &["title"]))
            .unwrap_or_default()
            .to_owned(),
        live_status,
        live_time,
        duration_sec,
        online,
        attention,
        area_name: first_non_empty_string(info_object, &["area_name"])
            .or_else(|| first_non_empty_string(status_object, &["area_v2_name", "area_name"]))
            .unwrap_or_default()
            .to_owned(),
        parent_area_name: first_non_empty_string(info_object, &["parent_area_name"])
            .or_else(|| {
                first_non_empty_string(status_object, &["area_v2_parent_name", "parent_area_name"])
            })
            .unwrap_or_default()
            .to_owned(),
        cover: first_non_empty_string(info_object, &["user_cover", "cover"])
            .or_else(|| first_non_empty_string(status_object, &["cover_from_user", "cover"]))
            .unwrap_or_default()
            .to_owned(),
        keyframe: first_non_empty_string(info_object, &["keyframe"])
            .or_else(|| first_non_empty_string(status_object, &["keyframe"]))
            .unwrap_or_default()
            .to_owned(),
        updated_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
    })
}

fn data_object<'a>(
    root: &'a Value,
    endpoint: &'static str,
) -> Result<&'a Map<String, Value>, NormalizationError> {
    root.get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| NormalizationError(format!("{endpoint} data is not an object")))
}

fn keyed_entry<'a>(
    data: &'a Map<String, Value>,
    uid: i64,
    endpoint: &'static str,
) -> Result<&'a Value, NormalizationError> {
    data.get(&uid.to_string())
        .or_else(|| {
            if data.len() == 1 {
                data.values().next()
            } else {
                None
            }
        })
        .ok_or_else(|| NormalizationError(format!("{endpoint} has no entry for uid {uid}")))
}

fn required_object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a Map<String, Value>, NormalizationError> {
    value
        .as_object()
        .ok_or_else(|| NormalizationError(format!("{label} is not an object")))
}

fn first_number(object: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .filter(|value| !value.is_null())
            .and_then(as_i64_lossy)
    })
}

fn first_string<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .filter(|value| !value.is_null())
            .and_then(as_str_lossy)
    })
}

fn first_non_empty_string<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .filter(|value| !value.is_null())
            .and_then(as_str_lossy)
            .filter(|value| !value.is_empty())
    })
}

fn as_i64_lossy(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    if let Some(number) = value.as_f64() {
        return number.is_finite().then(|| number.round() as i64);
    }

    let text = value.as_str()?.trim();
    if let Ok(number) = text.parse::<i64>() {
        return Some(number);
    }
    let number = text.parse::<f64>().ok()?;
    number.is_finite().then(|| number.round() as i64)
}

fn as_str_lossy(value: &Value) -> Option<&str> {
    value.as_str().map(str::trim)
}

fn normalize_live_status(value: i64) -> Option<i8> {
    match value {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

fn parse_live_time(value: &Value, now: i64) -> Option<i64> {
    match value {
        Value::Number(_) => normalize_epoch(value.as_f64()?.round() as i64, now),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() || text == "0000-00-00 00:00:00" || text == "0000-00-00" {
                return None;
            }

            if let Ok(timestamp) = text.parse::<i64>() {
                return normalize_epoch(timestamp, now);
            }
            if let Ok(timestamp) = text.parse::<f64>() {
                if timestamp.is_finite() {
                    return normalize_epoch(timestamp.round() as i64, now);
                }
            }
            if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
                return normalize_epoch(parsed.timestamp(), now);
            }
            if let Ok(parsed) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S") {
                // B 站的无时区时间字符串按中国标准时间（UTC+8）解释。
                let offset = FixedOffset::east_opt(8 * 60 * 60)?;
                let parsed = offset.from_local_datetime(&parsed).single()?;
                return normalize_epoch(parsed.timestamp(), now);
            }

            None
        }
        _ => None,
    }
}

fn normalize_epoch(timestamp: i64, now: i64) -> Option<i64> {
    if timestamp == -62_170_012_800 || timestamp <= 0 {
        return None;
    }

    let timestamp = if timestamp.unsigned_abs() >= 100_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    };

    (timestamp <= now + 86_400).then_some(timestamp)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn sources(status: Value, init: Value, info: Value) -> UpstreamSources {
        UpstreamSources { status, init, info }
    }

    #[test]
    fn maps_offline_sentinel_and_mixed_types() {
        let now = Utc.timestamp_opt(1_792_560_123, 0).unwrap();
        let sources = sources(
            serde_json::json!({
                "code": 0,
                "data": {
                    "85996232": {
                        "uid": 85996232_i64,
                        "room_id": 1890510937_i64,
                        "uname": "咿喔ioo_",
                        "title": "用户85996232的直播间",
                        "live_status": 0,
                        "live_time": "0",
                        "online": "0",
                        "area_v2_name": "颜值",
                        "area_v2_parent_name": "娱乐",
                        "cover_from_user": "",
                        "keyframe": ""
                    }
                }
            }),
            serde_json::json!({
                "code": 0,
                "data": {
                    "room_id": "1890510937",
                    "uid": "85996232",
                    "live_status": 0,
                    "live_time": -62_170_012_800_i64
                }
            }),
            serde_json::json!({
                "code": 0,
                "data": {
                    "uid": 85996232_i64,
                    "room_id": 1890510937_i64,
                    "attention": "570",
                    "online": 0.0,
                    "live_status": 0,
                    "live_time": "0000-00-00 00:00:00",
                    "area_name": "颜值",
                    "parent_area_name": "娱乐",
                    "user_cover": "",
                    "keyframe": ""
                }
            }),
        );

        let normalized = normalize(85996232, 1890510937, &sources, now).unwrap();
        assert_eq!(normalized.live_status, 0);
        assert_eq!(normalized.live_time, None);
        assert_eq!(normalized.duration_sec, 0);
        assert_eq!(normalized.online, 0);
        assert_eq!(normalized.attention, 570);
        assert_eq!(normalized.room_id, 1890510937);
        assert_eq!(
            normalized.updated_at,
            now.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
    }

    #[test]
    fn maps_live_status_and_computes_duration() {
        let now = Utc.timestamp_opt(1_792_560_123, 0).unwrap();
        let sources = sources(
            serde_json::json!({
                "code": 0,
                "data": {
                    "85996232": {
                        "uid": 85996232_u32,
                        "room_id": 1890510937_u32,
                        "uname": "咿喔ioo_",
                        "title": "直播中",
                        "live_status": 1,
                        "live_time": "1792560000",
                        "online": "123",
                        "area_v2_name": "颜值",
                        "area_v2_parent_name": "娱乐",
                        "cover_from_user": "cover.jpg",
                        "keyframe": "keyframe.jpg"
                    }
                }
            }),
            serde_json::json!({
                "code": 0,
                "data": {
                    "room_id": 1890510937_i64,
                    "uid": 85996232_i64,
                    "live_status": "1",
                    "live_time": -62_170_012_800_i64
                }
            }),
            serde_json::json!({
                "code": 0,
                "data": {
                    "uid": 85996232_i64,
                    "room_id": 1890510937_i64,
                    "attention": 570.0,
                    "online": 123,
                    "live_status": 1,
                    "live_time": "2026-10-20 00:00:00",
                    "area_name": "颜值",
                    "parent_area_name": "娱乐",
                    "user_cover": "info-cover.jpg",
                    "keyframe": "info-keyframe.jpg"
                }
            }),
        );

        let normalized = normalize(85996232, 1890510937, &sources, now).unwrap();
        assert_eq!(normalized.live_status, 1);
        assert_eq!(normalized.live_time, Some(1_792_560_000));
        assert_eq!(normalized.duration_sec, 123);
        assert_eq!(normalized.online, 123);
        assert_eq!(normalized.cover, "info-cover.jpg");
        assert_eq!(normalized.keyframe, "info-keyframe.jpg");
    }

    #[test]
    fn maps_round_live_status_without_duration() {
        let now = Utc.timestamp_opt(1_792_560_123, 0).unwrap();
        let sources = sources(
            serde_json::json!({
                "code": 0,
                "data": {
                    "85996232": {
                        "live_status": 2,
                        "live_time": 0,
                        "uname": "咿喔ioo_",
                        "title": "轮播"
                    }
                }
            }),
            serde_json::json!({
                "code": 0,
                "data": {"live_status": 2, "live_time": -62_170_012_800_i64}
            }),
            serde_json::json!({
                "code": 0,
                "data": {"live_status": "2", "live_time": "0000-00-00 00:00:00"}
            }),
        );

        let normalized = normalize(85996232, 1890510937, &sources, now).unwrap();
        assert_eq!(normalized.live_status, 2);
        assert_eq!(normalized.live_time, None);
        assert_eq!(normalized.duration_sec, 0);
    }
}
