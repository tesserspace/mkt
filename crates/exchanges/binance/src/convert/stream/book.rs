use mkt_core::Result;
use mkt_exchange_common as common;
use mkt_types::{BookTicker, OrderBook, OrderBookDelta, OrderBookLevel, Symbol};
use serde_json::Value;

use super::super::internal;

pub(crate) fn order_book_from_value(
    symbol: &Symbol,
    value: &Value,
    operation: &'static str,
) -> Result<OrderBook> {
    let response: binance_sdk::spot::websocket_streams::PartialBookDepthResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid partial book payload: {err}"))
        })?;

    OrderBook::builder()
        .symbol(symbol.clone())
        .bids(levels_from_response(response.bids, operation, "bids")?)
        .asks(levels_from_response(response.asks, operation, "asks")?)
        .last_update_id(response.last_update_id.map(|value| value.to_string()))
        .timestamp(None)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order_book", err.to_string()))
}

pub(crate) fn book_ticker_from_value(value: &Value, operation: &'static str) -> Result<BookTicker> {
    let response: binance_sdk::spot::websocket_streams::BookTickerResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid book ticker payload: {err}"))
        })?;

    BookTicker::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .bid_price(internal::parse_required_decimal(
            response.b, operation, "b",
        )?)
        .bid_quantity(internal::parse_required_decimal(
            response.b_uppercase,
            operation,
            "B",
        )?)
        .ask_price(internal::parse_required_decimal(
            response.a, operation, "a",
        )?)
        .ask_quantity(internal::parse_required_decimal(
            response.a_uppercase,
            operation,
            "A",
        )?)
        .last_update_id(response.u.map(|value| value.to_string()))
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "book_ticker", err.to_string()))
}

pub(crate) fn order_book_delta_from_value(
    value: &Value,
    operation: &'static str,
) -> Result<OrderBookDelta> {
    let response: binance_sdk::spot::websocket_streams::DiffBookDepthResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid diff depth payload: {err}"))
        })?;

    OrderBookDelta::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .first_update_id(response.u_uppercase.map(|value| value.to_string()))
        .last_update_id(response.u.map(|value| value.to_string()))
        .bids(levels_from_response(response.b, operation, "b")?)
        .asks(levels_from_response(response.a, operation, "a")?)
        .timestamp(internal::parse_optional_unix_millis_timestamp(
            response.e_uppercase,
            operation,
            "E",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order_book_delta", err.to_string()))
}

fn levels_from_response(
    levels: Option<Vec<Vec<String>>>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    levels
        .unwrap_or_default()
        .into_iter()
        .map(|level| {
            if level.len() < 2 {
                return Err(crate::error::invalid_field(
                    operation,
                    field,
                    "expected price/quantity level pair",
                ));
            }

            Ok(OrderBookLevel::new(
                common::parse_decimal(level[0].as_str()).map_err(|err| {
                    crate::error::invalid_field(operation, field, err.to_string())
                })?,
                common::parse_decimal(level[1].as_str()).map_err(|err| {
                    crate::error::invalid_field(operation, field, err.to_string())
                })?,
            ))
        })
        .collect()
}
