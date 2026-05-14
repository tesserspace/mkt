use mkt_core::Result;
use mkt_types::{Kline, KlineInterval, Symbol};
use serde_json::Value;

use super::super::internal;

pub(crate) fn stream_interval(
    interval: KlineInterval,
    operation: &'static str,
) -> Result<&'static str> {
    match interval {
        KlineInterval::Second(1) => Ok("1s"),
        KlineInterval::Minute(1) => Ok("1m"),
        KlineInterval::Minute(3) => Ok("3m"),
        KlineInterval::Minute(5) => Ok("5m"),
        KlineInterval::Minute(15) => Ok("15m"),
        KlineInterval::Minute(30) => Ok("30m"),
        KlineInterval::Hour(1) => Ok("1h"),
        KlineInterval::Hour(2) => Ok("2h"),
        KlineInterval::Hour(4) => Ok("4h"),
        KlineInterval::Hour(6) => Ok("6h"),
        KlineInterval::Hour(8) => Ok("8h"),
        KlineInterval::Hour(12) => Ok("12h"),
        KlineInterval::Day(1) => Ok("1d"),
        KlineInterval::Day(3) => Ok("3d"),
        KlineInterval::Week(1) => Ok("1w"),
        KlineInterval::Month(1) => Ok("1M"),
        _ => Err(crate::error::invalid_field(
            operation,
            "interval",
            "unsupported Binance spot kline stream interval",
        )),
    }
}

pub(crate) fn kline_from_value(
    value: &Value,
    interval: KlineInterval,
    operation: &'static str,
) -> Result<Kline> {
    let response: binance_sdk::spot::websocket_streams::KlineResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid kline payload: {err}"))
        })?;
    let kline = response
        .k
        .ok_or_else(|| crate::error::missing_field(operation, "k"))?;

    Kline::builder()
        .symbol(Symbol::spot(
            kline
                .s
                .or(response.s)
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .interval(interval)
        .open_time(internal::parse_required_unix_millis_timestamp(
            kline.t, operation, "k.t",
        )?)
        .close_time(internal::parse_required_unix_millis_timestamp(
            kline.t_uppercase,
            operation,
            "k.T",
        )?)
        .open(internal::parse_required_decimal(kline.o, operation, "k.o")?)
        .high(internal::parse_required_decimal(kline.h, operation, "k.h")?)
        .low(internal::parse_required_decimal(kline.l, operation, "k.l")?)
        .close(internal::parse_required_decimal(kline.c, operation, "k.c")?)
        .volume_base(internal::parse_required_decimal(kline.v, operation, "k.v")?)
        .volume_quote(Some(internal::parse_required_decimal(
            kline.q, operation, "k.q",
        )?))
        .closed(
            kline
                .x
                .ok_or_else(|| crate::error::missing_field(operation, "k.x"))?,
        )
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "kline", err.to_string()))
}
