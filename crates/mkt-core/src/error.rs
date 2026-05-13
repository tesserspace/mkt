use std::fmt;
use std::time::Duration;

use derive_builder::Builder;
use mkt_types::ExchangeId;
use strum_macros::{Display, EnumString, IntoStaticStr};
use thiserror::Error;

use crate::Capability;

pub type Result<T> = std::result::Result<T, Error>;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityUnavailableReason {
    NotAdvertised,
    NotBound,
}

impl fmt::Display for CapabilityUnavailableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAdvertised => f.write_str("capability is not advertised by this adapter"),
            Self::NotBound => f.write_str("capability is advertised but not bound into the handle"),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct InvalidConfigContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub config_key: Option<String>,
    pub message: String,
}

impl InvalidConfigContext {
    pub fn builder() -> InvalidConfigContextBuilder {
        InvalidConfigContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct BadRequestContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    pub message: String,
}

impl BadRequestContext {
    pub fn builder() -> BadRequestContextBuilder {
        BadRequestContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct TransportContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    #[builder(default)]
    pub status: Option<u16>,
    pub message: String,
}

impl TransportContext {
    pub fn builder() -> TransportContextBuilder {
        TransportContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct DecodeContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    pub message: String,
}

impl DecodeContext {
    pub fn builder() -> DecodeContextBuilder {
        DecodeContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct AuthenticationContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    #[builder(default)]
    pub code: Option<String>,
    pub message: String,
}

impl AuthenticationContext {
    pub fn builder() -> AuthenticationContextBuilder {
        AuthenticationContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct TimeoutContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    pub message: String,
}

impl TimeoutContext {
    pub fn builder() -> TimeoutContextBuilder {
        TimeoutContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct RateLimitedContext {
    #[builder(default)]
    pub exchange: Option<ExchangeId>,
    #[builder(default)]
    pub operation: Option<String>,
    #[builder(default)]
    pub retry_after: Option<Duration>,
    pub message: String,
}

impl RateLimitedContext {
    pub fn builder() -> RateLimitedContextBuilder {
        RateLimitedContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct ExchangeErrorContext {
    pub exchange: ExchangeId,
    #[builder(default)]
    pub operation: Option<String>,
    #[builder(default)]
    pub code: Option<String>,
    #[builder(default)]
    pub status: Option<u16>,
    pub message: String,
}

impl ExchangeErrorContext {
    pub fn builder() -> ExchangeErrorContextBuilder {
        ExchangeErrorContextBuilder::default()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum ErrorKind {
    CapabilityUnavailable,
    MissingCredentials,
    InvalidConfig,
    BadRequest,
    Transport,
    Decode,
    Authentication,
    Timeout,
    RateLimited,
    Exchange,
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("{exchange} cannot provide {capability}: {reason}")]
    CapabilityUnavailable {
        exchange: ExchangeId,
        capability: Capability,
        reason: CapabilityUnavailableReason,
    },
    #[error("missing API credentials for {exchange}")]
    MissingCredentials { exchange: ExchangeId },
    #[error(
        "invalid config{exchange}{config_key}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        config_key = suffix::config_key(.context.config_key.as_deref()),
    )]
    InvalidConfig { context: InvalidConfigContext },
    #[error(
        "bad request{exchange}{operation}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
    )]
    BadRequest { context: BadRequestContext },
    #[error(
        "transport error{exchange}{operation}{status}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
        status = suffix::status(.context.status),
    )]
    Transport { context: TransportContext },
    #[error(
        "decode error{exchange}{operation}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
    )]
    Decode { context: DecodeContext },
    #[error(
        "authentication error{exchange}{operation}{code}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
        code = suffix::code(.context.code.as_deref()),
    )]
    Authentication { context: AuthenticationContext },
    #[error(
        "request timed out{exchange}{operation}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
    )]
    Timeout { context: TimeoutContext },
    #[error(
        "rate limited{exchange}{operation}{retry_after}: {}",
        .context.message,
        exchange = suffix::exchange(.context.exchange.as_ref()),
        operation = suffix::operation(.context.operation.as_deref()),
        retry_after = suffix::retry_after(.context.retry_after),
    )]
    RateLimited { context: RateLimitedContext },
    #[error(
        "exchange error for {exchange}{operation}{code}{status}: {}",
        .context.message,
        exchange = .context.exchange,
        operation = suffix::operation(.context.operation.as_deref()),
        code = suffix::code(.context.code.as_deref()),
        status = suffix::status(.context.status),
    )]
    Exchange { context: ExchangeErrorContext },
}

macro_rules! impl_context_error_conversions {
    ($context:ident, $builder:ident, $variant:ident, $expect:literal) => {
        impl From<$context> for Error {
            fn from(context: $context) -> Self {
                Self::$variant { context }
            }
        }

        impl From<$builder> for Error {
            fn from(builder: $builder) -> Self {
                Self::$variant {
                    context: builder.build().expect($expect),
                }
            }
        }
    };
}

impl_context_error_conversions!(
    InvalidConfigContext,
    InvalidConfigContextBuilder,
    InvalidConfig,
    "invalid config builder must include a message"
);
impl_context_error_conversions!(
    BadRequestContext,
    BadRequestContextBuilder,
    BadRequest,
    "bad request builder must include a message"
);
impl_context_error_conversions!(
    TransportContext,
    TransportContextBuilder,
    Transport,
    "transport builder must include a message"
);
impl_context_error_conversions!(
    DecodeContext,
    DecodeContextBuilder,
    Decode,
    "decode builder must include a message"
);
impl_context_error_conversions!(
    AuthenticationContext,
    AuthenticationContextBuilder,
    Authentication,
    "authentication builder must include a message"
);
impl_context_error_conversions!(
    TimeoutContext,
    TimeoutContextBuilder,
    Timeout,
    "timeout builder must include a message"
);
impl_context_error_conversions!(
    RateLimitedContext,
    RateLimitedContextBuilder,
    RateLimited,
    "rate-limited builder must include a message"
);
impl_context_error_conversions!(
    ExchangeErrorContext,
    ExchangeErrorContextBuilder,
    Exchange,
    "exchange error builder must include exchange and message"
);

impl Error {
    pub fn capability_unavailable(
        exchange: ExchangeId,
        capability: Capability,
        reason: CapabilityUnavailableReason,
    ) -> Self {
        Self::CapabilityUnavailable {
            exchange,
            capability,
            reason,
        }
    }

    pub fn unsupported(exchange: ExchangeId, capability: Capability) -> Self {
        Self::capability_unavailable(
            exchange,
            capability,
            CapabilityUnavailableReason::NotAdvertised,
        )
    }

    pub fn missing_credentials(exchange: ExchangeId) -> Self {
        Self::MissingCredentials { exchange }
    }

    pub fn invalid_config(message: impl Into<String>) -> InvalidConfigContextBuilder {
        InvalidConfigContext::builder().message(message)
    }

    pub fn bad_request(message: impl Into<String>) -> BadRequestContextBuilder {
        BadRequestContext::builder().message(message)
    }

    pub fn transport(message: impl Into<String>) -> TransportContextBuilder {
        TransportContext::builder().message(message)
    }

    pub fn decode(message: impl Into<String>) -> DecodeContextBuilder {
        DecodeContext::builder().message(message)
    }

    pub fn authentication(message: impl Into<String>) -> AuthenticationContextBuilder {
        AuthenticationContext::builder().message(message)
    }

    pub fn timeout(message: impl Into<String>) -> TimeoutContextBuilder {
        TimeoutContext::builder().message(message)
    }

    pub fn rate_limited(message: impl Into<String>) -> RateLimitedContextBuilder {
        RateLimitedContext::builder().message(message)
    }

    pub fn exchange_error(
        exchange: ExchangeId,
        message: impl Into<String>,
    ) -> ExchangeErrorContextBuilder {
        ExchangeErrorContext::builder()
            .exchange(exchange)
            .message(message)
    }

    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::CapabilityUnavailable { .. } => ErrorKind::CapabilityUnavailable,
            Self::MissingCredentials { .. } => ErrorKind::MissingCredentials,
            Self::InvalidConfig { .. } => ErrorKind::InvalidConfig,
            Self::BadRequest { .. } => ErrorKind::BadRequest,
            Self::Transport { .. } => ErrorKind::Transport,
            Self::Decode { .. } => ErrorKind::Decode,
            Self::Authentication { .. } => ErrorKind::Authentication,
            Self::Timeout { .. } => ErrorKind::Timeout,
            Self::RateLimited { .. } => ErrorKind::RateLimited,
            Self::Exchange { .. } => ErrorKind::Exchange,
        }
    }

    pub fn exchange(&self) -> Option<&ExchangeId> {
        match self {
            Self::CapabilityUnavailable { exchange, .. }
            | Self::MissingCredentials { exchange } => Some(exchange),
            Self::InvalidConfig { context } => context.exchange.as_ref(),
            Self::BadRequest { context } => context.exchange.as_ref(),
            Self::Transport { context } => context.exchange.as_ref(),
            Self::Decode { context } => context.exchange.as_ref(),
            Self::Authentication { context } => context.exchange.as_ref(),
            Self::Timeout { context } => context.exchange.as_ref(),
            Self::RateLimited { context } => context.exchange.as_ref(),
            Self::Exchange { context } => Some(&context.exchange),
        }
    }

    pub fn operation(&self) -> Option<&str> {
        match self {
            Self::BadRequest { context } => context.operation.as_deref(),
            Self::Transport { context } => context.operation.as_deref(),
            Self::Decode { context } => context.operation.as_deref(),
            Self::Authentication { context } => context.operation.as_deref(),
            Self::Timeout { context } => context.operation.as_deref(),
            Self::RateLimited { context } => context.operation.as_deref(),
            Self::Exchange { context } => context.operation.as_deref(),
            Self::CapabilityUnavailable { .. }
            | Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. } => None,
        }
    }

    pub fn config_key(&self) -> Option<&str> {
        match self {
            Self::InvalidConfig { context } => context.config_key.as_deref(),
            Self::CapabilityUnavailable { .. }
            | Self::MissingCredentials { .. }
            | Self::BadRequest { .. }
            | Self::Transport { .. }
            | Self::Decode { .. }
            | Self::Authentication { .. }
            | Self::Timeout { .. }
            | Self::RateLimited { .. }
            | Self::Exchange { .. } => None,
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::InvalidConfig { context } => Some(context.message.as_str()),
            Self::BadRequest { context } => Some(context.message.as_str()),
            Self::Transport { context } => Some(context.message.as_str()),
            Self::Decode { context } => Some(context.message.as_str()),
            Self::Authentication { context } => Some(context.message.as_str()),
            Self::Timeout { context } => Some(context.message.as_str()),
            Self::RateLimited { context } => Some(context.message.as_str()),
            Self::Exchange { context } => Some(context.message.as_str()),
            Self::CapabilityUnavailable { .. } | Self::MissingCredentials { .. } => None,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Transport { context } => context.status,
            Self::Exchange { context } => context.status,
            Self::CapabilityUnavailable { .. }
            | Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. }
            | Self::BadRequest { .. }
            | Self::Decode { .. }
            | Self::Authentication { .. }
            | Self::Timeout { .. }
            | Self::RateLimited { .. } => None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Authentication { context } => context.code.as_deref(),
            Self::Exchange { context } => context.code.as_deref(),
            Self::CapabilityUnavailable { .. }
            | Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. }
            | Self::BadRequest { .. }
            | Self::Transport { .. }
            | Self::Decode { .. }
            | Self::Timeout { .. }
            | Self::RateLimited { .. } => None,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { context } => context.retry_after,
            Self::CapabilityUnavailable { .. }
            | Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. }
            | Self::BadRequest { .. }
            | Self::Transport { .. }
            | Self::Decode { .. }
            | Self::Authentication { .. }
            | Self::Timeout { .. }
            | Self::Exchange { .. } => None,
        }
    }

    pub fn capability(&self) -> Option<Capability> {
        match self {
            Self::CapabilityUnavailable { capability, .. } => Some(*capability),
            Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. }
            | Self::BadRequest { .. }
            | Self::Transport { .. }
            | Self::Decode { .. }
            | Self::Authentication { .. }
            | Self::Timeout { .. }
            | Self::RateLimited { .. }
            | Self::Exchange { .. } => None,
        }
    }

    pub fn capability_reason(&self) -> Option<CapabilityUnavailableReason> {
        match self {
            Self::CapabilityUnavailable { reason, .. } => Some(*reason),
            Self::MissingCredentials { .. }
            | Self::InvalidConfig { .. }
            | Self::BadRequest { .. }
            | Self::Transport { .. }
            | Self::Decode { .. }
            | Self::Authentication { .. }
            | Self::Timeout { .. }
            | Self::RateLimited { .. }
            | Self::Exchange { .. } => None,
        }
    }
}

mod suffix {
    use std::time::Duration;

    use mkt_types::ExchangeId;

    pub(super) fn exchange(exchange: Option<&ExchangeId>) -> String {
        exchange
            .map(|exchange| format!(" for {exchange}"))
            .unwrap_or_default()
    }

    pub(super) fn operation(operation: Option<&str>) -> String {
        operation
            .map(|operation| format!(" during {operation}"))
            .unwrap_or_default()
    }

    pub(super) fn config_key(config_key: Option<&str>) -> String {
        config_key
            .map(|config_key| format!(" at {config_key}"))
            .unwrap_or_default()
    }

    pub(super) fn status(status: Option<u16>) -> String {
        status
            .map(|status| format!(" (status {status})"))
            .unwrap_or_default()
    }

    pub(super) fn code(code: Option<&str>) -> String {
        code.map(|code| format!(" [code={code}]"))
            .unwrap_or_default()
    }

    pub(super) fn retry_after(retry_after: Option<Duration>) -> String {
        retry_after
            .map(|retry_after| format!(" [retry_after={}s]", retry_after.as_secs()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mkt_types::{ExchangeId, KnownExchange};

    use super::{CapabilityUnavailableReason, Error, ErrorKind};
    use crate::Capability;

    #[test]
    fn timeout_and_transport_stay_distinct_for_error_classification() {
        let exchange = ExchangeId::from(KnownExchange::Binance);
        let timeout: Error = Error::timeout("socket stalled waiting for the next frame")
            .exchange(exchange.clone())
            .operation("private_stream.read")
            .into();
        let transport: Error = Error::transport("connection reset by peer")
            .exchange(exchange.clone())
            .operation("private_stream.read")
            .status(503_u16)
            .into();

        assert_eq!(timeout.kind(), ErrorKind::Timeout);
        assert_eq!(transport.kind(), ErrorKind::Transport);
        assert_eq!(timeout.exchange(), Some(&exchange));
        assert_eq!(timeout.operation(), Some("private_stream.read"));
        assert_eq!(transport.status(), Some(503));
        assert_eq!(
            timeout.message(),
            Some("socket stalled waiting for the next frame")
        );
    }

    #[test]
    fn structured_fields_survive_without_reparsing_display_text() {
        let exchange = ExchangeId::from(KnownExchange::Okx);
        let err: Error = Error::exchange_error(exchange.clone(), "system busy")
            .operation("spot.place_order")
            .code("30084")
            .status(429_u16)
            .into();
        let unsupported = Error::capability_unavailable(
            exchange.clone(),
            Capability::PublicStream,
            CapabilityUnavailableReason::NotBound,
        );

        assert_eq!(err.kind(), ErrorKind::Exchange);
        assert_eq!(err.exchange(), Some(&exchange));
        assert_eq!(err.operation(), Some("spot.place_order"));
        assert_eq!(err.code(), Some("30084"));
        assert_eq!(err.status(), Some(429));
        assert_eq!(err.message(), Some("system busy"));

        assert_eq!(unsupported.kind(), ErrorKind::CapabilityUnavailable);
        assert_eq!(unsupported.capability(), Some(Capability::PublicStream));
        assert_eq!(
            unsupported.capability_reason(),
            Some(CapabilityUnavailableReason::NotBound)
        );
        let rate_limited: Error = Error::rate_limited("slow down")
            .retry_after(Duration::from_secs(3))
            .into();
        assert_eq!(rate_limited.retry_after(), Some(Duration::from_secs(3)));
    }

    #[test]
    fn invalid_config_is_not_a_request_error() {
        let exchange = ExchangeId::from(KnownExchange::Binance);
        let err: Error = Error::invalid_config("relative URL without a base")
            .exchange(exchange.clone())
            .config_key("config.rest")
            .into();

        assert_eq!(err.kind(), ErrorKind::InvalidConfig);
        assert_eq!(err.exchange(), Some(&exchange));
        assert_eq!(err.operation(), None);
        assert_eq!(err.config_key(), Some("config.rest"));
        assert_eq!(err.message(), Some("relative URL without a base"));
        assert_eq!(err.status(), None);
    }
}
