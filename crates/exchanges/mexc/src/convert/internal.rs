use std::str::FromStr;

use mkt_core::Result;
use mkt_types::{KlineInterval, MarketKind, Symbol};
use rust_decimal::Decimal;
use time::OffsetDateTime;

pub(super) fn parse_decimal(
    raw: String,
    operation: &'static str,
    field: &'static str,
) -> Result<Decimal> {
    Decimal::from_str(raw.as_str())
        .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))
}

pub(super) fn parse_required_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Decimal> {
    parse_decimal(
        raw.ok_or_else(|| crate::error::missing_field(operation, field))?,
        operation,
        field,
    )
}

pub(super) fn parse_optional_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<Decimal>> {
    raw.map(|value| parse_decimal(value, operation, field))
        .transpose()
}

pub(super) fn parse_required_i64(
    raw: Option<i64>,
    operation: &'static str,
    field: &'static str,
) -> Result<i64> {
    raw.ok_or_else(|| crate::error::missing_field(operation, field))
}

pub(super) fn parse_optional_i64(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<i64>> {
    raw.map(|value| {
        value
            .parse::<i64>()
            .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))
    })
    .transpose()
}

pub(crate) fn unix_timestamp_millis(
    timestamp: OffsetDateTime,
    operation: &'static str,
    field: &'static str,
) -> Result<i64> {
    let timestamp_millis = timestamp.unix_timestamp_nanos() / 1_000_000;
    i64::try_from(timestamp_millis)
        .map_err(|_| crate::error::invalid_field(operation, field, "timestamp out of i64 range"))
}

pub(super) fn parse_unix_millis_timestamp(
    timestamp_millis: i64,
    operation: &'static str,
    field: &'static str,
) -> Result<OffsetDateTime> {
    if timestamp_millis < 0 {
        return Err(crate::error::invalid_field(
            operation,
            field,
            "invalid Unix millisecond timestamp",
        ));
    }

    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_millis) * 1_000_000).map_err(
        |_| crate::error::invalid_field(operation, field, "invalid Unix millisecond timestamp"),
    )
}

pub(super) fn parse_value_decimal(
    value: &serde_json::Value,
    operation: &'static str,
    field: &'static str,
) -> Result<Decimal> {
    parse_decimal(value_to_string(value.clone()), operation, field)
}

pub(super) fn parse_value_timestamp(
    value: &serde_json::Value,
    operation: &'static str,
    field: &'static str,
) -> Result<OffsetDateTime> {
    let raw = match value {
        serde_json::Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| crate::error::invalid_field(operation, field, "timestamp is not i64"))?,
        serde_json::Value::String(raw) => raw
            .parse::<i64>()
            .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))?,
        other => {
            return Err(crate::error::invalid_field(
                operation,
                field,
                format!("unsupported timestamp value `{other}`"),
            ))
        }
    };
    parse_unix_millis_timestamp(raw, operation, field)
}

pub(super) fn value_to_string(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value,
        other => other.to_string(),
    }
}

pub(super) fn closed_from_close_time(close_time: OffsetDateTime) -> bool {
    close_time < OffsetDateTime::now_utc()
}

pub(crate) fn require_spot_symbol(symbol: &Symbol, operation: &'static str) -> Result<String> {
    if !matches!(symbol.kind, MarketKind::Spot) {
        return Err(crate::error::invalid_field(
            operation,
            "symbol",
            format!(
                "MEXC spot workflow only accepts spot symbols, got `{}`",
                symbol.kind
            ),
        ));
    }
    Ok(symbol.venue_symbol.clone())
}

pub(crate) fn mexc_interval(
    interval: KlineInterval,
    operation: &'static str,
) -> Result<&'static str> {
    match interval {
        KlineInterval::Minute(1) => Ok("1m"),
        KlineInterval::Minute(5) => Ok("5m"),
        KlineInterval::Minute(15) => Ok("15m"),
        KlineInterval::Minute(30) => Ok("30m"),
        KlineInterval::Hour(1) => Ok("60m"),
        KlineInterval::Hour(4) => Ok("4h"),
        KlineInterval::Day(1) => Ok("1d"),
        KlineInterval::Week(1) => Ok("1W"),
        KlineInterval::Month(1) => Ok("1M"),
        _ => Err(crate::error::invalid_field(
            operation,
            "interval",
            "unsupported MEXC spot kline interval",
        )),
    }
}
