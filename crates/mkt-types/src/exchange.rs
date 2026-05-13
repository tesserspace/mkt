use std::fmt;

use strum_macros::{Display, EnumString, IntoStaticStr};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Display, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case", ascii_case_insensitive)]
pub enum KnownExchange {
    Binance,
    Bybit,
    Bitget,
    Mexc,
    Okx,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionExchangeId(String);

impl ExtensionExchangeId {
    pub fn new(value: impl Into<String>) -> Result<Self, ExchangeIdParseError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ExchangeIdParseError::Empty);
        }

        let valid_format = value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if valid_format {
            Ok(Self(value))
        } else {
            Err(ExchangeIdParseError::InvalidExtensionId(value))
        }
    }
}

impl fmt::Display for ExtensionExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeIdParseError {
    Empty,
    InvalidExtensionId(String),
}

impl fmt::Display for ExchangeIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("exchange id cannot be empty"),
            Self::InvalidExtensionId(value) => write!(f, "invalid extension exchange id: {value}"),
        }
    }
}

impl std::error::Error for ExchangeIdParseError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExchangeId {
    Known(KnownExchange),
    Extension(ExtensionExchangeId),
}

impl ExchangeId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(id) => (*id).into(),
            Self::Extension(id) => &id.0,
        }
    }
}

impl From<KnownExchange> for ExchangeId {
    fn from(value: KnownExchange) -> Self {
        Self::Known(value)
    }
}

impl TryFrom<&str> for ExchangeId {
    type Error = ExchangeIdParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.parse::<KnownExchange>() {
            Ok(id) => Ok(Self::Known(id)),
            Err(_) => ExtensionExchangeId::new(value).map(Self::Extension),
        }
    }
}

impl TryFrom<String> for ExchangeId {
    type Error = ExchangeIdParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{ExchangeId, ExchangeIdParseError, KnownExchange};

    #[test]
    fn known_exchange_ids_are_normalized() {
        assert_eq!(
            ExchangeId::try_from("OKX").expect("known exchange"),
            ExchangeId::Known(KnownExchange::Okx)
        );
    }

    #[test]
    fn extension_exchange_ids_must_be_canonical() {
        let err = ExchangeId::try_from("MyDesk").expect_err("uppercase extension id should fail");

        assert!(matches!(err, ExchangeIdParseError::InvalidExtensionId(_)));
        assert_eq!(
            ExchangeId::try_from("my-desk").expect("canonical extension id"),
            ExchangeId::Extension(super::ExtensionExchangeId::new("my-desk").expect("valid id"))
        );
    }
}
