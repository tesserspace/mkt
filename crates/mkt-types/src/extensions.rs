use std::borrow::Borrow;
use std::collections::{btree_map, BTreeMap};
use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::{Number, Value};
use time::OffsetDateTime;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NamespaceKey(String);

impl NamespaceKey {
    pub fn new(value: impl Into<String>) -> Result<Self, NamespaceKeyError> {
        let value = value.into();
        Self::validate(value.as_str())?;

        Ok(Self(value))
    }

    pub fn validate(value: &str) -> Result<(), NamespaceKeyError> {
        let Some((namespace, name)) = value.split_once('.') else {
            return Err(NamespaceKeyError::MissingNamespace(value.to_owned()));
        };

        if namespace.is_empty() || name.is_empty() {
            return Err(NamespaceKeyError::EmptySegment(value.to_owned()));
        }

        if namespace.contains('.') || name.contains('.') {
            return Err(NamespaceKeyError::TooManySegments(value.to_owned()));
        }

        if !(namespace.bytes().all(Self::is_namespace_segment_byte)
            && name.bytes().all(Self::is_namespace_segment_byte))
        {
            return Err(NamespaceKeyError::InvalidCharacters(value.to_owned()));
        }

        Ok(())
    }

    fn is_namespace_segment_byte(byte: u8) -> bool {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions(BTreeMap<NamespaceKey, Value>);

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: impl AsRef<str>) -> Option<&Value> {
        self.0.get(key.as_ref())
    }

    pub fn iter(&self) -> btree_map::Iter<'_, NamespaceKey, Value> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_map(&self) -> &BTreeMap<NamespaceKey, Value> {
        &self.0
    }

    pub fn into_map(self) -> BTreeMap<NamespaceKey, Value> {
        self.0
    }

    pub fn insert(&mut self, key: impl AsRef<str>, value: Value) -> Result<(), NamespaceKeyError> {
        let namespace_key = NamespaceKey::new(key.as_ref())?;
        self.0.insert(namespace_key, value);
        Ok(())
    }

    pub fn insert_optional_string(
        &mut self,
        key: impl AsRef<str>,
        value: Option<String>,
    ) -> Result<(), NamespaceKeyError> {
        if let Some(value) = value {
            self.insert(key, Value::String(value))?;
        }
        Ok(())
    }

    pub fn insert_optional_bool(
        &mut self,
        key: impl AsRef<str>,
        value: Option<bool>,
    ) -> Result<(), NamespaceKeyError> {
        if let Some(value) = value {
            self.insert(key, Value::Bool(value))?;
        }
        Ok(())
    }

    pub fn insert_i64(
        &mut self,
        key: impl AsRef<str>,
        value: i64,
    ) -> Result<(), NamespaceKeyError> {
        self.insert(key, Value::Number(Number::from(value)))
    }

    pub fn string(&self, key: impl AsRef<str>) -> Result<Option<String>, ExtensionValueError> {
        let key = key.as_ref();
        let Some(value) = self.0.get(key) else {
            return Ok(None);
        };
        match value {
            Value::String(value) => Ok(Some(value.clone())),
            other => Err(ExtensionValueError::invalid_type(
                key,
                "string",
                other.to_string(),
            )),
        }
    }

    pub fn i64(&self, key: impl AsRef<str>) -> Result<Option<i64>, ExtensionValueError> {
        let key = key.as_ref();
        let Some(value) = self.0.get(key) else {
            return Ok(None);
        };
        match value {
            Value::Number(number) => number
                .as_i64()
                .ok_or_else(|| {
                    ExtensionValueError::invalid_type(
                        key,
                        "integer value that fits into i64",
                        number.to_string(),
                    )
                })
                .map(Some),
            Value::String(raw) => {
                raw.parse::<i64>()
                    .map(Some)
                    .map_err(|err| ExtensionValueError::ParseInteger {
                        key: key.to_owned(),
                        value: raw.clone(),
                        message: err.to_string(),
                    })
            }
            other => Err(ExtensionValueError::invalid_type(
                key,
                "integer",
                other.to_string(),
            )),
        }
    }

    pub fn i32(&self, key: impl AsRef<str>) -> Result<Option<i32>, ExtensionValueError> {
        let key = key.as_ref();
        self.i64(key)?
            .map(|value| {
                i32::try_from(value).map_err(|_| ExtensionValueError::IntegerOutOfRange {
                    key: key.to_owned(),
                    value,
                    target: "i32",
                })
            })
            .transpose()
    }

    pub fn decimal(&self, key: impl AsRef<str>) -> Result<Option<Decimal>, ExtensionValueError> {
        let key = key.as_ref();
        let Some(value) = self.0.get(key) else {
            return Ok(None);
        };

        let raw = match value {
            Value::String(value) => value.clone(),
            Value::Number(number) => number.to_string(),
            other => {
                return Err(ExtensionValueError::invalid_type(
                    key,
                    "decimal string or number",
                    other.to_string(),
                ))
            }
        };

        Decimal::from_str(&raw)
            .map(Some)
            .map_err(|err| ExtensionValueError::ParseDecimal {
                key: key.to_owned(),
                value: raw,
                message: err.to_string(),
            })
    }

    pub fn parse_optional_decimal<E>(
        value: Option<String>,
        on_error: impl FnOnce(rust_decimal::Error) -> E,
    ) -> Result<Option<Decimal>, E> {
        value
            .map(|raw| Decimal::from_str(raw.as_ref()).map_err(on_error))
            .transpose()
    }

    pub fn parse_timestamp<E>(
        value: i64,
        on_error: impl FnOnce(time::error::ComponentRange) -> E,
    ) -> Result<OffsetDateTime, E> {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(value) * 1_000_000).map_err(on_error)
    }

    pub fn parse_optional_timestamp<E>(
        value: Option<i64>,
        on_error: impl FnOnce(time::error::ComponentRange) -> E,
    ) -> Result<Option<OffsetDateTime>, E> {
        value
            .filter(|timestamp| *timestamp >= 0)
            .map(|timestamp| Self::parse_timestamp(timestamp, on_error))
            .transpose()
    }
}

impl From<BTreeMap<NamespaceKey, Value>> for Extensions {
    fn from(value: BTreeMap<NamespaceKey, Value>) -> Self {
        Self(value)
    }
}

impl From<Extensions> for BTreeMap<NamespaceKey, Value> {
    fn from(value: Extensions) -> Self {
        value.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionValueError {
    InvalidType {
        key: String,
        expected: &'static str,
        actual: String,
    },
    ParseInteger {
        key: String,
        value: String,
        message: String,
    },
    IntegerOutOfRange {
        key: String,
        value: i64,
        target: &'static str,
    },
    ParseDecimal {
        key: String,
        value: String,
        message: String,
    },
}

impl ExtensionValueError {
    fn invalid_type(key: &str, expected: &'static str, actual: String) -> Self {
        Self::InvalidType {
            key: key.to_owned(),
            expected,
            actual,
        }
    }
}

impl TryFrom<&str> for NamespaceKey {
    type Error = NamespaceKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for NamespaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Borrow<str> for NamespaceKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NamespaceKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NamespaceKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceKeyError {
    MissingNamespace(String),
    EmptySegment(String),
    TooManySegments(String),
    InvalidCharacters(String),
}

impl fmt::Display for ExtensionValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidType {
                key,
                expected,
                actual,
            } => write!(f, "extension `{key}` expected {expected}, got {actual}"),
            Self::ParseInteger {
                key,
                value,
                message,
            } => write!(
                f,
                "extension `{key}` could not parse integer `{value}`: {message}"
            ),
            Self::IntegerOutOfRange { key, value, target } => {
                write!(
                    f,
                    "extension `{key}` integer `{value}` does not fit into {target}"
                )
            }
            Self::ParseDecimal {
                key,
                value,
                message,
            } => write!(
                f,
                "extension `{key}` could not parse decimal `{value}`: {message}"
            ),
        }
    }
}

impl std::error::Error for ExtensionValueError {}

impl fmt::Display for NamespaceKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNamespace(value) => {
                write!(f, "namespace key must be namespace.name: {value}")
            }
            Self::EmptySegment(value) => write!(f, "namespace key has an empty segment: {value}"),
            Self::TooManySegments(value) => {
                write!(f, "namespace key must contain exactly one dot: {value}")
            }
            Self::InvalidCharacters(value) => {
                write!(f, "namespace key contains invalid characters: {value}")
            }
        }
    }
}

impl std::error::Error for NamespaceKeyError {}
