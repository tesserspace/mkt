use std::str::FromStr;

use mkt_core::Result;
use mkt_types::{MiniTicker, Symbol};
use rust_decimal::Decimal;

use crate::{
    error,
    protobuf::{PublicMiniTickerV3Api, PublicMiniTickersV3Api},
};

pub(crate) fn mini_ticker_from_proto(
    expected_symbol: &Symbol,
    payload: PublicMiniTickerV3Api,
    operation: &'static str,
) -> Result<MiniTicker> {
    if !payload.symbol.is_empty() && payload.symbol != expected_symbol.venue_symbol {
        return Err(error::invalid_field(
            operation,
            "symbol",
            format!(
                "MEXC mini ticker payload symbol `{}` did not match subscription `{}`",
                payload.symbol, expected_symbol.venue_symbol
            ),
        ));
    }

    mini_ticker_from_fields(expected_symbol.clone(), payload, operation)
}

pub(crate) fn mini_tickers_from_batch(
    expected_symbol: &Symbol,
    payload: PublicMiniTickersV3Api,
    operation: &'static str,
) -> Result<Vec<MiniTicker>> {
    payload
        .items
        .into_iter()
        .filter(|item| item.symbol == expected_symbol.venue_symbol)
        .map(|item| mini_ticker_from_fields(expected_symbol.clone(), item, operation))
        .collect()
}

fn mini_ticker_from_fields(
    symbol: Symbol,
    payload: PublicMiniTickerV3Api,
    operation: &'static str,
) -> Result<MiniTicker> {
    let close = parse_decimal(payload.price, operation, "price")?;
    MiniTicker::builder()
        .symbol(symbol)
        .close(close)
        .open(close)
        .high(parse_decimal(payload.high, operation, "high")?)
        .low(parse_decimal(payload.low, operation, "low")?)
        .volume_base(parse_decimal(payload.quantity, operation, "quantity")?)
        .volume_quote(parse_decimal(payload.volume, operation, "volume")?)
        .build()
        .map_err(|err| error::invalid_field(operation, "mini_ticker", err.to_string()))
}

fn parse_decimal(raw: String, operation: &'static str, field: &'static str) -> Result<Decimal> {
    Decimal::from_str(raw.as_str())
        .map_err(|err| error::invalid_field(operation, field, err.to_string()))
}
