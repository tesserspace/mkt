pub(crate) use helper::{
    adapter_error, decode_error, invalid_field, map_connector_error, map_request_error,
    missing_field, websocket_error,
};

mod helper {
    use anyhow::Error as AnyhowError;
    use binance_sdk::common::errors::ConnectorError;
    use mkt_core::Error;
    use mkt_types::{ExchangeId, KnownExchange};

    pub(crate) fn adapter_error(operation: &'static str, message: impl Into<String>) -> Error {
        Error::exchange_error(ExchangeId::from(KnownExchange::Binance), message.into())
            .operation(operation)
            .into()
    }

    pub(crate) fn missing_field(operation: &'static str, field: &'static str) -> Error {
        adapter_error(
            operation,
            format!("missing required Binance field `{field}`"),
        )
    }

    pub(crate) fn invalid_field(
        operation: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Error {
        adapter_error(operation, format!("invalid `{field}`: {}", message.into()))
    }

    pub(crate) fn decode_error(operation: &'static str, message: impl Into<String>) -> Error {
        Error::decode(message.into())
            .exchange(ExchangeId::from(KnownExchange::Binance))
            .operation(operation)
            .into()
    }

    pub(crate) fn websocket_error(operation: &'static str, message: impl Into<String>) -> Error {
        Error::transport(message.into())
            .exchange(ExchangeId::from(KnownExchange::Binance))
            .operation(operation)
            .into()
    }

    pub(crate) fn map_request_error(operation: &'static str, err: AnyhowError) -> Error {
        match err.downcast::<ConnectorError>() {
            Ok(connector) => map_connector_error(operation, connector),
            Err(other) => Error::transport(other.to_string())
                .exchange(ExchangeId::from(KnownExchange::Binance))
                .operation(operation)
                .into(),
        }
    }

    pub(crate) fn map_connector_error(operation: &'static str, err: ConnectorError) -> Error {
        match err {
            ConnectorError::BadRequestError { msg, code } => Error::bad_request(match code {
                Some(code) => format!("HTTP 400, Binance code {code}: {msg}"),
                None => format!("HTTP 400: {msg}"),
            })
            .exchange(ExchangeId::from(KnownExchange::Binance))
            .operation(operation)
            .into(),
            ConnectorError::UnauthorizedError { msg, code } => {
                let mut builder = Error::authentication(msg)
                    .exchange(ExchangeId::from(KnownExchange::Binance))
                    .operation(operation);
                if let Some(code) = code {
                    builder = builder.code(code.to_string());
                }
                builder.into()
            }
            ConnectorError::ForbiddenError { msg, code } => {
                let mut builder = Error::authentication(msg)
                    .exchange(ExchangeId::from(KnownExchange::Binance))
                    .operation(operation);
                if let Some(code) = code {
                    builder = builder.code(code.to_string());
                }
                builder.into()
            }
            ConnectorError::TooManyRequestsError { msg, .. }
            | ConnectorError::RateLimitBanError { msg, .. } => Error::rate_limited(msg)
                .exchange(ExchangeId::from(KnownExchange::Binance))
                .operation(operation)
                .into(),
            ConnectorError::ServerError { msg, status_code } => {
                let mut builder = Error::transport(msg)
                    .exchange(ExchangeId::from(KnownExchange::Binance))
                    .operation(operation);
                if let Some(status_code) = status_code {
                    builder = builder.status(status_code);
                }
                builder.into()
            }
            ConnectorError::NetworkError(message) => Error::transport(message)
                .exchange(ExchangeId::from(KnownExchange::Binance))
                .operation(operation)
                .into(),
            ConnectorError::NotFoundError { msg, code } => Error::bad_request(match code {
                Some(code) => format!("HTTP 404, Binance code {code}: {msg}"),
                None => format!("HTTP 404: {msg}"),
            })
            .exchange(ExchangeId::from(KnownExchange::Binance))
            .operation(operation)
            .into(),
            ConnectorError::ConnectorClientError { msg, code } => {
                if let Some(code) = code {
                    return Error::exchange_error(ExchangeId::from(KnownExchange::Binance), msg)
                        .operation(operation)
                        .code(code.to_string())
                        .into();
                }

                Error::transport(msg)
                    .exchange(ExchangeId::from(KnownExchange::Binance))
                    .operation(operation)
                    .into()
            }
        }
    }
}
