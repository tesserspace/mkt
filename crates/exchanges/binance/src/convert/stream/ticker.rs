use mkt_core::Result;
use mkt_types::{AveragePrice, MiniTicker, Symbol};
use serde_json::Value;

use super::super::internal;

pub(super) fn average_price_from_value(
    value: &Value,
    operation: &'static str,
) -> Result<AveragePrice> {
    let response: binance_sdk::spot::websocket_streams::AvgPriceResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid average price payload: {err}"))
        })?;

    AveragePrice::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .interval(response.i)
        .price(internal::parse_required_decimal(
            response.w, operation, "w",
        )?)
        .event_time(internal::parse_optional_unix_millis_timestamp(
            response.e_uppercase,
            operation,
            "E",
        )?)
        .last_trade_time(internal::parse_optional_unix_millis_timestamp(
            response.t_uppercase,
            operation,
            "T",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "average_price", err.to_string()))
}

pub(super) fn mini_ticker_from_value(value: &Value, operation: &'static str) -> Result<MiniTicker> {
    let response: binance_sdk::spot::websocket_streams::MiniTickerResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid mini ticker payload: {err}"))
        })?;

    MiniTicker::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .close(internal::parse_required_decimal(
            response.c, operation, "c",
        )?)
        .open(internal::parse_required_decimal(
            response.o, operation, "o",
        )?)
        .high(internal::parse_required_decimal(
            response.h, operation, "h",
        )?)
        .low(internal::parse_required_decimal(
            response.l, operation, "l",
        )?)
        .volume_base(internal::parse_required_decimal(
            response.v, operation, "v",
        )?)
        .volume_quote(internal::parse_required_decimal(
            response.q, operation, "q",
        )?)
        .event_time(internal::parse_optional_unix_millis_timestamp(
            response.e_uppercase,
            operation,
            "E",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "mini_ticker", err.to_string()))
}
