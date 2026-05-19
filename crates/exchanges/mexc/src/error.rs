pub(crate) use helper::{decode_error, invalid_field, map_http_error, missing_field};

mod helper {
    use std::time::Duration;

    use mkt_core::Error;
    use mkt_types::{ExchangeId, KnownExchange};
    use reqwest::{header::HeaderValue, Error as ReqwestError, StatusCode};

    use crate::rest::MexcApiErrorPayload;

    pub(crate) fn adapter_error(operation: &'static str, message: impl Into<String>) -> Error {
        Error::exchange_error(ExchangeId::from(KnownExchange::Mexc), message.into())
            .operation(operation)
            .into()
    }

    pub(crate) fn invalid_field(
        operation: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Error {
        adapter_error(operation, format!("invalid `{field}`: {}", message.into()))
    }

    pub(crate) fn missing_field(operation: &'static str, field: &'static str) -> Error {
        adapter_error(operation, format!("missing required MEXC field `{field}`"))
    }

    pub(crate) fn decode_error(operation: &'static str, message: impl Into<String>) -> Error {
        Error::decode(message.into())
            .exchange(ExchangeId::from(KnownExchange::Mexc))
            .operation(operation)
            .into()
    }

    pub(crate) fn map_http_error(
        operation: &'static str,
        status: Option<StatusCode>,
        payload: Option<MexcApiErrorPayload>,
        retry_after: Option<HeaderValue>,
        transport: Option<ReqwestError>,
    ) -> Error {
        if let Some(err) = transport {
            if err.is_timeout() {
                return Error::timeout(err.to_string())
                    .exchange(ExchangeId::from(KnownExchange::Mexc))
                    .operation(operation)
                    .into();
            }

            return Error::transport(err.to_string())
                .exchange(ExchangeId::from(KnownExchange::Mexc))
                .operation(operation)
                .into();
        }

        if let Some(status) = status {
            let message = payload
                .as_ref()
                .map(MexcApiErrorPayload::message)
                .unwrap_or_else(|| status.to_string());
            let code = payload.as_ref().map(MexcApiErrorPayload::code_string);
            match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    let mut builder = Error::authentication(message)
                        .exchange(ExchangeId::from(KnownExchange::Mexc))
                        .operation(operation);
                    if let Some(code) = code {
                        builder = builder.code(code);
                    }
                    return builder.into();
                }
                StatusCode::IM_A_TEAPOT | StatusCode::TOO_MANY_REQUESTS => {
                    let message = match code {
                        Some(code) => format!("{message} (MEXC code {code})"),
                        None => message,
                    };
                    let mut builder = Error::rate_limited(message)
                        .exchange(ExchangeId::from(KnownExchange::Mexc))
                        .operation(operation);
                    builder = builder.retry_after(
                        retry_after
                            .as_ref()
                            .and_then(retry_after_from_header)
                            .unwrap_or_else(|| Duration::from_secs(1)),
                    );
                    return builder.into();
                }
                _ if status.is_client_error() => {
                    let mut builder = Error::bad_request(message.clone())
                        .exchange(ExchangeId::from(KnownExchange::Mexc))
                        .operation(operation);
                    if let Some(code) = code {
                        builder = builder.message(format!("{message} (MEXC code {code})"));
                    }
                    return builder.into();
                }
                _ => {
                    let message = match code {
                        Some(code) => format!("{message} (MEXC code {code})"),
                        None => message,
                    };
                    return Error::transport(message)
                        .exchange(ExchangeId::from(KnownExchange::Mexc))
                        .operation(operation)
                        .status(status.as_u16())
                        .into();
                }
            }
        }

        adapter_error(operation, "missing HTTP error context")
    }

    fn retry_after_from_header(header: &HeaderValue) -> Option<Duration> {
        let raw = header.to_str().ok()?.trim();
        if raw.is_empty() {
            return None;
        }

        raw.parse::<u64>().ok().map(Duration::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mkt_core::ErrorKind;
    use reqwest::{header::HeaderValue, StatusCode};

    use crate::rest::MexcApiErrorPayload;

    use super::map_http_error;

    const OPERATION: &str = "rest.test";

    #[test]
    fn http_429_maps_to_rate_limited_with_retry_after_header() {
        let err = map_http_error(
            OPERATION,
            Some(StatusCode::TOO_MANY_REQUESTS),
            Some(MexcApiErrorPayload {
                code: Some(serde_json::json!(700003)),
                msg: Some("Too many requests".to_owned()),
            }),
            Some(HeaderValue::from_static("7")),
            None,
        );

        assert_eq!(err.kind(), ErrorKind::RateLimited);
        assert_eq!(err.operation(), Some(OPERATION));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
        assert_eq!(err.message(), Some("Too many requests (MEXC code 700003)"));
    }

    #[test]
    fn http_418_maps_to_rate_limited_and_falls_back_to_one_second() {
        let err = map_http_error(
            OPERATION,
            Some(StatusCode::IM_A_TEAPOT),
            Some(MexcApiErrorPayload {
                code: Some(serde_json::json!("BAN")),
                msg: Some("IP temporarily banned".to_owned()),
            }),
            Some(HeaderValue::from_static("not-a-duration")),
            None,
        );

        assert_eq!(err.kind(), ErrorKind::RateLimited);
        assert_eq!(err.retry_after(), Some(Duration::from_secs(1)));
        assert_eq!(err.message(), Some("IP temporarily banned (MEXC code BAN)"));
    }
}
