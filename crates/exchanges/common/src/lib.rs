use std::{fmt::Display, str::FromStr};

use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use sha2::Sha256;
use time::OffsetDateTime;
use url::Url;

type HmacSha256 = Hmac<Sha256>;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDecimalError {
    message: String,
}

impl ParseDecimalError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParseDecimalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseDecimalError {}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampError {
    message: String,
}

impl TimestampError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for TimestampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TimestampError {}

pub fn parse_decimal(raw: &str) -> Result<Decimal, ParseDecimalError> {
    Decimal::from_str(raw).map_err(|err| ParseDecimalError {
        message: err.to_string(),
    })
}

pub fn parse_optional_decimal(raw: Option<String>) -> Result<Option<Decimal>, ParseDecimalError> {
    raw.map(|value| parse_decimal(value.as_str())).transpose()
}

pub fn parse_unix_millis_timestamp(
    timestamp_millis: i64,
) -> Result<OffsetDateTime, TimestampError> {
    if timestamp_millis < 0 {
        return Err(TimestampError {
            message: "invalid Unix millisecond timestamp".to_owned(),
        });
    }

    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_millis) * 1_000_000).map_err(
        |_| TimestampError {
            message: "invalid Unix millisecond timestamp".to_owned(),
        },
    )
}

pub fn parse_unix_seconds_timestamp(
    timestamp_seconds: i64,
) -> Result<OffsetDateTime, TimestampError> {
    if timestamp_seconds < 0 {
        return Err(TimestampError {
            message: "invalid Unix second timestamp".to_owned(),
        });
    }

    OffsetDateTime::from_unix_timestamp(timestamp_seconds).map_err(|_| TimestampError {
        message: "invalid Unix second timestamp".to_owned(),
    })
}

pub fn parse_optional_unix_millis_timestamp(
    raw: Option<i64>,
) -> Result<Option<OffsetDateTime>, TimestampError> {
    raw.map(parse_unix_millis_timestamp).transpose()
}

pub fn unix_timestamp_millis(timestamp: OffsetDateTime) -> Result<i64, TimestampError> {
    let timestamp_millis = timestamp.unix_timestamp_nanos() / 1_000_000;
    i64::try_from(timestamp_millis).map_err(|_| TimestampError {
        message: "timestamp out of i64 range".to_owned(),
    })
}

pub fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

pub fn closed_from_close_time(close_time: OffsetDateTime) -> bool {
    close_time < OffsetDateTime::now_utc()
}

pub fn closed_from_unix_seconds(timestamp_seconds: i64) -> bool {
    parse_unix_seconds_timestamp(timestamp_seconds)
        .is_ok_and(|close_time| close_time <= OffsetDateTime::now_utc())
}

pub fn hmac_sha256_hex(secret: &[u8], payload: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|err| err.to_string())?;
    mac.update(payload.as_bytes());
    Ok(hex_lower(mac.finalize().into_bytes().as_slice()))
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut result, "{byte:02x}");
    }
    result
}

pub fn parse_base_url(raw: &str, trailing_slash: bool) -> Result<Url, String> {
    let mut parsed = Url::parse(raw).map_err(|err| err.to_string())?;
    if trailing_slash && !parsed.path().ends_with('/') {
        parsed.set_path(&format!("{}/", parsed.path()));
    }
    Ok(parsed)
}

pub fn serialize_query<K, V>(query: &[(K, V)]) -> String
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in query {
        serializer.append_pair(key.as_ref(), value.as_ref());
    }
    serializer.finish()
}

pub fn query_pair(key: &'static str, value: impl Display) -> (&'static str, String) {
    (key, value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        closed_from_unix_seconds, hmac_sha256_hex, parse_base_url, parse_unix_millis_timestamp,
        parse_unix_seconds_timestamp, serialize_query, value_to_string,
    };
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    #[test]
    fn value_to_string_preserves_string_without_json_quotes() {
        assert_eq!(value_to_string(&json!("abc")), "abc");
        assert_eq!(value_to_string(&json!(42)), "42");
    }

    #[test]
    fn unix_millis_rejects_negative_timestamp() {
        let err = parse_unix_millis_timestamp(-1).expect_err("negative timestamp should fail");
        assert_eq!(err.message(), "invalid Unix millisecond timestamp");
    }

    #[test]
    fn unix_seconds_rejects_negative_timestamp() {
        let err = parse_unix_seconds_timestamp(-1).expect_err("negative timestamp should fail");
        assert_eq!(err.message(), "invalid Unix second timestamp");
    }

    #[test]
    fn closed_from_unix_seconds_rejects_invalid_and_compares_to_now() {
        assert!(!closed_from_unix_seconds(-1));
        assert!(closed_from_unix_seconds(
            (OffsetDateTime::now_utc() - Duration::seconds(1)).unix_timestamp()
        ));
        assert!(!closed_from_unix_seconds(
            (OffsetDateTime::now_utc() + Duration::seconds(60)).unix_timestamp()
        ));
    }

    #[test]
    fn hmac_sha256_matches_known_vector() {
        assert_eq!(
            hmac_sha256_hex(b"key", "payload").expect("HMAC should compute"),
            "5d98b45c90a207fa998ce639fea6f02ecc8cc3f36fef81d694fb856b4d0a28ca"
        );
    }

    #[test]
    fn parse_base_url_adds_trailing_slash_when_requested() {
        assert_eq!(
            parse_base_url("https://api.example.com/root", true)
                .expect("URL should parse")
                .as_str(),
            "https://api.example.com/root/"
        );
    }

    #[test]
    fn serialize_query_url_encodes_pairs() {
        let query = [
            ("symbol", "USDCUSDT".to_owned()),
            ("newClientOrderId", "client id".to_owned()),
        ];
        assert_eq!(
            serialize_query(&query),
            "symbol=USDCUSDT&newClientOrderId=client+id"
        );
    }
}
