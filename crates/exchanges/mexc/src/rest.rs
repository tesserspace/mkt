use std::fmt::Display;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use mkt_core::{Error, ExposeSecret, Result};
use reqwest::{header::RETRY_AFTER, Method, Response};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sha2::Sha256;
use time::OffsetDateTime;
use url::Url;

use crate::{error, MexcInner};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub(crate) struct MexcRestClient {
    inner: Arc<MexcInner>,
}

impl MexcRestClient {
    pub(crate) fn new(inner: Arc<MexcInner>) -> Self {
        Self { inner }
    }

    pub(crate) async fn get_public<T>(
        &self,
        operation: &'static str,
        path: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .request(Method::GET, path, query, false)
            .map_err(|err| self.invalid_config_error("request", err))?;
        self.send_json(operation, request).await
    }

    pub(crate) async fn get_signed<T>(
        &self,
        operation: &'static str,
        path: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .request(Method::GET, path, query, true)
            .map_err(|err| self.invalid_config_error("request", err))?;
        self.send_json(operation, request).await
    }

    pub(crate) async fn post_signed<T>(
        &self,
        operation: &'static str,
        path: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .request(Method::POST, path, query, true)
            .map_err(|err| self.invalid_config_error("request", err))?;
        self.send_json(operation, request).await
    }

    pub(crate) async fn delete_signed<T>(
        &self,
        operation: &'static str,
        path: &'static str,
        query: Vec<(&'static str, String)>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .request(Method::DELETE, path, query, true)
            .map_err(|err| self.invalid_config_error("request", err))?;
        self.send_json(operation, request).await
    }

    fn request(
        &self,
        method: Method,
        path: &'static str,
        query: Vec<(&'static str, String)>,
        signed: bool,
    ) -> std::result::Result<reqwest::RequestBuilder, String> {
        let timestamp = signed.then(unix_timestamp_millis).transpose()?;
        self.request_with_timestamp(method, path, query, timestamp)
    }

    fn request_with_timestamp(
        &self,
        method: Method,
        path: &'static str,
        mut query: Vec<(&'static str, String)>,
        signed_timestamp: Option<i64>,
    ) -> std::result::Result<reqwest::RequestBuilder, String> {
        if let Some(timestamp) = signed_timestamp {
            query.push(("timestamp", timestamp.to_string()));
        }
        let mut url = self
            .inner
            .rest_base_url
            .join(path)
            .map_err(|err| err.to_string())?;
        let query_string = serialize_query(&query)?;
        if signed_timestamp.is_some() {
            let signature = self.sign(&query_string)?;
            url.set_query(Some(
                format!("{query_string}&signature={signature}").as_str(),
            ));
        } else if !query_string.is_empty() {
            url.set_query(Some(query_string.as_str()));
        }

        let mut builder = self.inner.http.request(method, url);
        if let (Some(_), Some(credentials)) = (signed_timestamp, &self.inner.config.credentials) {
            builder = builder.header("X-MEXC-APIKEY", credentials.api_key().expose_secret());
        }
        Ok(builder)
    }

    async fn send_json<T>(
        &self,
        operation: &'static str,
        request: reqwest::RequestBuilder,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = request
            .send()
            .await
            .map_err(|err| error::map_http_error(operation, None, None, None, Some(err)))?;
        parse_json_response(operation, response).await
    }

    fn sign(&self, payload: &str) -> std::result::Result<String, String> {
        let credentials = self
            .inner
            .config
            .credentials
            .as_ref()
            .ok_or_else(|| "missing credentials".to_owned())?;
        let mut mac = HmacSha256::new_from_slice(credentials.secret().expose_secret().as_bytes())
            .map_err(|err| err.to_string())?;
        mac.update(payload.as_bytes());
        let signature = mac.finalize().into_bytes();
        Ok(hex_lower(signature.as_slice()))
    }

    fn invalid_config_error(&self, config_key: &'static str, message: String) -> Error {
        Error::invalid_config(message)
            .exchange(self.inner.exchange_id())
            .config_key(config_key)
            .into()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MexcApiErrorPayload {
    #[serde(default)]
    pub(crate) code: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) msg: Option<String>,
}

impl MexcApiErrorPayload {
    pub(crate) fn from_error_body(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes).ok().or_else(|| {
            let raw = String::from_utf8_lossy(bytes).trim().to_owned();
            (!raw.is_empty()).then_some(Self {
                code: None,
                msg: Some(raw),
            })
        })
    }

    pub(crate) fn message(&self) -> String {
        self.msg
            .clone()
            .unwrap_or_else(|| "unknown MEXC error".to_owned())
    }

    pub(crate) fn code_string(&self) -> String {
        match &self.code {
            Some(serde_json::Value::String(value)) => value.clone(),
            Some(value) => value.to_string(),
            None => "unknown".to_owned(),
        }
    }
}

async fn parse_json_response<T>(operation: &'static str, response: Response) -> Result<T>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let retry_after = response.headers().get(RETRY_AFTER).cloned();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| error::map_http_error(operation, Some(status), None, None, Some(err)))?;
    if status.is_success() {
        return serde_json::from_slice(&bytes)
            .map_err(|err| error::decode_error(operation, err.to_string()));
    }

    let payload = MexcApiErrorPayload::from_error_body(&bytes);
    Err(error::map_http_error(
        operation,
        Some(status),
        payload,
        retry_after,
        None,
    ))
}

fn serialize_query(query: &[(&'static str, String)]) -> std::result::Result<String, String> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in query {
        serializer.append_pair(key, value);
    }
    Ok(serializer.finish())
}

fn unix_timestamp_millis() -> std::result::Result<i64, String> {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| "timestamp is out of i64 millisecond range".to_owned())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut result, "{byte:02x}");
    }
    result
}

pub(crate) fn base_rest_url(url: Option<&str>) -> std::result::Result<Url, String> {
    let raw = url.unwrap_or("https://api.mexc.com");
    let mut parsed = Url::parse(raw).map_err(|err| err.to_string())?;
    if !parsed.path().ends_with('/') {
        parsed.set_path(&format!("{}/", parsed.path()));
    }
    Ok(parsed)
}

pub(crate) fn base_websocket_url(url: Option<&str>) -> std::result::Result<Url, String> {
    Url::parse(url.unwrap_or("wss://wbs-api.mexc.com/ws")).map_err(|err| err.to_string())
}

pub(crate) fn query_pair(key: &'static str, value: impl Display) -> (&'static str, String) {
    (key, value.to_string())
}

#[cfg(test)]
mod tests {
    use super::MexcApiErrorPayload;

    #[test]
    fn api_error_payload_preserves_non_json_body_text() {
        let payload = MexcApiErrorPayload::from_error_body(b"upstream gateway failure")
            .expect("non-empty raw error body should be preserved");

        assert_eq!(payload.message(), "upstream gateway failure");
        assert_eq!(payload.code, None);
    }

    #[test]
    fn api_error_payload_parses_json_code_and_message() {
        let payload =
            MexcApiErrorPayload::from_error_body(br#"{"code":700003,"msg":"Too many requests"}"#)
                .expect("JSON MEXC error body should parse");

        assert_eq!(payload.message(), "Too many requests");
        assert_eq!(payload.code_string(), "700003");
    }
}
