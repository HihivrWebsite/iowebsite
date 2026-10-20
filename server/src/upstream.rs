use std::{fmt, future::Future};

use axum::http::{
    header::{ACCEPT, COOKIE, REFERER, USER_AGENT},
    HeaderValue,
};
use reqwest::{Client, StatusCode};
use serde_json::Value;

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const STATUS_URL: &str = "https://api.live.bilibili.com/room/v1/Room/get_status_info_by_uids";
const ROOM_INIT_URL: &str = "https://api.live.bilibili.com/room/v1/Room/room_init";
const ROOM_INFO_URL: &str = "https://api.live.bilibili.com/room/v1/Room/get_info";

#[derive(Debug, Clone)]
pub struct UpstreamClient {
    client: Client,
    default_headers: reqwest::header::HeaderMap,
}

#[derive(Debug)]
pub struct UpstreamSources {
    pub status: Value,
    pub init: Value,
    pub info: Value,
}

#[derive(Debug)]
pub enum UpstreamError {
    BuildClient(reqwest::Error),
    Http {
        endpoint: &'static str,
        source: reqwest::Error,
    },
    HttpStatus {
        endpoint: &'static str,
        status: StatusCode,
    },
    ApiResponse {
        endpoint: &'static str,
        code: i64,
        message: String,
    },
    MissingData {
        endpoint: &'static str,
    },
    InvalidReferer {
        room_id: i64,
    },
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildClient(error) => {
                write!(formatter, "failed to build upstream client: {error}")
            }
            Self::Http { endpoint, source } => {
                write!(formatter, "{endpoint} request failed: {source}")
            }
            Self::HttpStatus { endpoint, status } => {
                write!(formatter, "{endpoint} returned HTTP {status}")
            }
            Self::ApiResponse {
                endpoint,
                code,
                message,
            } => write!(formatter, "{endpoint} returned code {code}: {message}"),
            Self::MissingData { endpoint } => write!(formatter, "{endpoint} returned no data"),
            Self::InvalidReferer { room_id } => {
                write!(formatter, "invalid room id for Referer: {room_id}")
            }
        }
    }
}

impl std::error::Error for UpstreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BuildClient(error) => Some(error),
            Self::Http { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl UpstreamClient {
    pub fn new() -> Result<Self, UpstreamError> {
        let mut default_headers = reqwest::header::HeaderMap::new();
        default_headers.insert(USER_AGENT, HeaderValue::from_static(BROWSER_USER_AGENT));
        default_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        debug_assert!(!default_headers.contains_key(COOKIE));

        let client = Client::builder()
            .default_headers(default_headers.clone())
            .connect_timeout(std::time::Duration::from_secs(2))
            .timeout(std::time::Duration::from_secs(4))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(UpstreamError::BuildClient)?;

        Ok(Self {
            client,
            default_headers,
        })
    }

    pub fn has_cookie_header(&self) -> bool {
        self.default_headers.contains_key(COOKIE)
    }

    pub async fn fetch_sources(
        &self,
        uid: i64,
        room_id: i64,
    ) -> Result<UpstreamSources, UpstreamError> {
        let referer_value = HeaderValue::from_str(&format!("https://live.bilibili.com/{room_id}"))
            .map_err(|_| UpstreamError::InvalidReferer { room_id })?;

        let status_request = self
            .client
            .get(STATUS_URL)
            .query(&[("uids[]", uid)])
            .header(REFERER, referer_value.clone())
            .send();
        let init_request = self
            .client
            .get(ROOM_INIT_URL)
            .query(&[("id", room_id)])
            .header(REFERER, referer_value.clone())
            .send();

        // get_status_info_by_uids 与 room_init 是首选上游；先并发请求，
        // 再用 get_info 补充 attention、parent_area_name 等字段。
        let (status, init) = tokio::join!(
            fetch_json("get_status_info_by_uids", status_request),
            fetch_json("room_init", init_request)
        );
        let status = status?;
        let init = init?;

        let info_request = self
            .client
            .get(ROOM_INFO_URL)
            .query(&[("room_id", room_id)])
            .header(REFERER, referer_value)
            .send();
        let info = fetch_json("get_info", info_request).await?;

        Ok(UpstreamSources { status, init, info })
    }
}

async fn fetch_json(
    endpoint: &'static str,
    request: impl Future<Output = reqwest::Result<reqwest::Response>>,
) -> Result<Value, UpstreamError> {
    let response = request
        .await
        .map_err(|source| UpstreamError::Http { endpoint, source })?;

    let status = response.status();
    if !status.is_success() {
        return Err(UpstreamError::HttpStatus { endpoint, status });
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|source| UpstreamError::Http { endpoint, source })?;

    let code = payload
        .get("code")
        .and_then(Value::as_i64)
        .ok_or(UpstreamError::MissingData { endpoint })?;
    if code != 0 {
        let message = payload
            .get("message")
            .or_else(|| payload.get("msg"))
            .and_then(Value::as_str)
            .unwrap_or("unknown upstream error")
            .to_owned();
        return Err(UpstreamError::ApiResponse {
            endpoint,
            code,
            message,
        });
    }

    if payload.get("data").is_none_or(Value::is_null) {
        return Err(UpstreamError::MissingData { endpoint });
    }

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upstream_client_never_sets_cookie_by_default() {
        let client = UpstreamClient::new().expect("client should build");
        assert!(!client.has_cookie_header());
    }
}
