use derive_builder::Builder;
use rust_decimal::Decimal;
use std::{fmt, str::FromStr};
use strum_macros::{Display, EnumString};
use time::OffsetDateTime;

use crate::{Extensions, Symbol};

/// Kline (candlestick) interval expressed as a unit plus count.
///
/// Common fixed intervals have convenience constants (e.g.,
/// [`KlineInterval::M1`], [`KlineInterval::H1`]), but calendar intervals such
/// as months and years are represented explicitly so they do not get forced
/// into an imprecise second-based duration.
///
/// `Display` and `FromStr` use a neutral canonical form such as `"1m"`,
/// `"1M"`, or `"2h"`. That representation is not guaranteed to be accepted by
/// any specific exchange API; adapters must translate it to the venue's
/// expected parameter format before sending a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KlineInterval {
    Second(u64),
    Minute(u64),
    Hour(u64),
    Day(u64),
    Week(u64),
    Month(u64),
    Year(u64),
}

impl KlineInterval {
    pub const M1: Self = Self::Minute(1);
    pub const M3: Self = Self::Minute(3);
    pub const M5: Self = Self::Minute(5);
    pub const M15: Self = Self::Minute(15);
    pub const M30: Self = Self::Minute(30);
    pub const H1: Self = Self::Hour(1);
    pub const H4: Self = Self::Hour(4);
    pub const D1: Self = Self::Day(1);
    pub const W1: Self = Self::Week(1);
    pub const MO1: Self = Self::Month(1);
    pub const Y1: Self = Self::Year(1);

    /// Supports arbitrary granularity; adapters map this into exchange-specific strings.
    pub const fn from_secs(secs: u64) -> Self {
        if secs == 0 {
            Self::Second(0)
        } else if secs % 604800 == 0 {
            Self::Week(secs / 604800)
        } else if secs % 86400 == 0 {
            Self::Day(secs / 86400)
        } else if secs % 3600 == 0 {
            Self::Hour(secs / 3600)
        } else if secs % 60 == 0 {
            Self::Minute(secs / 60)
        } else {
            Self::Second(secs)
        }
    }
}

impl FromStr for KlineInterval {
    type Err = IntervalParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (number, unit) = if let Some(number) = value.strip_suffix('s') {
            (number, Self::Second(0))
        } else if let Some(number) = value.strip_suffix('m') {
            (number, Self::Minute(0))
        } else if let Some(number) = value.strip_suffix('h') {
            (number, Self::Hour(0))
        } else if let Some(number) = value.strip_suffix('d') {
            (number, Self::Day(0))
        } else if let Some(number) = value.strip_suffix('w') {
            (number, Self::Week(0))
        } else if let Some(number) = value.strip_suffix('M') {
            (number, Self::Month(0))
        } else if let Some(number) = value.strip_suffix('y') {
            (number, Self::Year(0))
        } else {
            return Err(IntervalParseError {
                input: value.to_owned(),
            });
        };

        let amount: u64 = number.parse().map_err(|_| IntervalParseError {
            input: value.to_owned(),
        })?;
        Ok(match unit {
            Self::Second(_) => Self::Second(amount),
            Self::Minute(_) => Self::Minute(amount),
            Self::Hour(_) => Self::Hour(amount),
            Self::Day(_) => Self::Day(amount),
            Self::Week(_) => Self::Week(amount),
            Self::Month(_) => Self::Month(amount),
            Self::Year(_) => Self::Year(amount),
        })
    }
}

impl fmt::Display for KlineInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Second(count) => write!(f, "{count}s"),
            Self::Minute(count) => write!(f, "{count}m"),
            Self::Hour(count) => write!(f, "{count}h"),
            Self::Day(count) => write!(f, "{count}d"),
            Self::Week(count) => write!(f, "{count}w"),
            Self::Month(count) => write!(f, "{count}M"),
            Self::Year(count) => write!(f, "{count}y"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalParseError {
    input: String,
}

impl fmt::Display for IntervalParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid kline interval: {}", self.input)
    }
}

impl std::error::Error for IntervalParseError {}

#[cfg(feature = "serde")]
impl serde::Serialize for KlineInterval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for KlineInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct LastPrice {
    pub symbol: Symbol,
    pub price: Decimal,
}

impl LastPrice {
    pub fn new(symbol: Symbol, price: Decimal) -> Self {
        Self { symbol, price }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

impl OrderBookLevel {
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct OrderBook {
    pub symbol: Symbol,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    #[builder(default)]
    pub last_update_id: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds::option")
    )]
    #[builder(default)]
    pub timestamp: Option<OffsetDateTime>,
    #[builder(default)]
    pub extensions: Extensions,
}

impl OrderBook {
    pub fn builder() -> OrderBookBuilder {
        OrderBookBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Kline {
    pub symbol: Symbol,
    pub interval: KlineInterval,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub open_time: OffsetDateTime,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub close_time: OffsetDateTime,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume_base: Decimal,
    #[builder(default)]
    pub volume_quote: Option<Decimal>,
    pub closed: bool,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Kline {
    pub fn builder() -> KlineBuilder {
        KlineBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct KlineRequest {
    pub symbol: Symbol,
    pub interval: KlineInterval,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds::option")
    )]
    #[builder(default)]
    pub start: Option<OffsetDateTime>,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds::option")
    )]
    #[builder(default)]
    pub end: Option<OffsetDateTime>,
    #[builder(default)]
    pub limit: Option<u32>,
}

impl KlineRequest {
    pub fn builder() -> KlineRequestBuilder {
        KlineRequestBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Trade {
    pub symbol: Symbol,
    #[builder(default)]
    pub id: Option<String>,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: TradeSide,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub timestamp: OffsetDateTime,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Trade {
    pub fn builder() -> TradeBuilder {
        TradeBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use super::KlineInterval;

    #[test]
    fn common_intervals_parse_to_duration_backed_values() {
        assert_eq!("1m".parse::<KlineInterval>(), Ok(KlineInterval::M1));
        assert_eq!("1h".parse::<KlineInterval>(), Ok(KlineInterval::H1));
        assert_eq!("1M".parse::<KlineInterval>(), Ok(KlineInterval::MO1));
    }

    #[test]
    fn month_intervals_round_trip_through_display() {
        let interval = "3M".parse::<KlineInterval>().expect("valid interval");

        assert_eq!(interval, KlineInterval::Month(3));
        assert_eq!(interval.to_string(), "3M");
    }
}
