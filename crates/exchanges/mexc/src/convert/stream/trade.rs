use mkt_core::Result;
use mkt_exchange_common as common;
use mkt_types::{AggTrade, LastPrice, Symbol, Trade, TradeSide};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::{
    error,
    protobuf::{PublicAggreDealsV3Api, PublicAggreDealsV3ApiItem},
};

pub(crate) fn trades_from_aggre_deals(
    symbol: Symbol,
    payload: PublicAggreDealsV3Api,
    operation: &'static str,
) -> Result<Vec<Trade>> {
    payload
        .deals
        .into_iter()
        .map(|deal| trade_from_deal(symbol.clone(), deal, operation))
        .collect()
}

pub(crate) fn last_prices_from_aggre_deals(
    symbol: Symbol,
    payload: PublicAggreDealsV3Api,
    operation: &'static str,
) -> Result<Vec<LastPrice>> {
    payload
        .deals
        .into_iter()
        .map(|deal| {
            Ok(LastPrice::new(
                symbol.clone(),
                parse_price(deal.price, operation)?,
            ))
        })
        .collect()
}

pub(crate) fn agg_trades_from_aggre_deals(
    symbol: Symbol,
    payload: PublicAggreDealsV3Api,
    operation: &'static str,
) -> Result<Vec<AggTrade>> {
    payload
        .deals
        .into_iter()
        .map(|deal| agg_trade_from_deal(symbol.clone(), deal, operation))
        .collect()
}

fn trade_from_deal(
    symbol: Symbol,
    deal: PublicAggreDealsV3ApiItem,
    operation: &'static str,
) -> Result<Trade> {
    Trade::builder()
        .symbol(symbol)
        .price(parse_price(deal.price, operation)?)
        .quantity(parse_quantity(deal.quantity, operation)?)
        .side(side_from_trade_type(deal.trade_type, operation)?)
        .timestamp(parse_unix_millis_timestamp(deal.time, operation)?)
        .build()
        .map_err(|err| error::invalid_field(operation, "trade", err.to_string()))
}

fn agg_trade_from_deal(
    symbol: Symbol,
    deal: PublicAggreDealsV3ApiItem,
    operation: &'static str,
) -> Result<AggTrade> {
    AggTrade::builder()
        .symbol(symbol)
        .price(parse_price(deal.price, operation)?)
        .quantity(parse_quantity(deal.quantity, operation)?)
        .side(side_from_trade_type(deal.trade_type, operation)?)
        .timestamp(parse_unix_millis_timestamp(deal.time, operation)?)
        .build()
        .map_err(|err| error::invalid_field(operation, "agg_trade", err.to_string()))
}

fn parse_price(raw: String, operation: &'static str) -> Result<Decimal> {
    parse_decimal(raw, operation, "price")
}

fn parse_quantity(raw: String, operation: &'static str) -> Result<Decimal> {
    parse_decimal(raw, operation, "quantity")
}

fn parse_decimal(raw: String, operation: &'static str, field: &'static str) -> Result<Decimal> {
    common::parse_decimal(raw.as_str())
        .map_err(|err| error::invalid_field(operation, field, err.to_string()))
}

fn side_from_trade_type(trade_type: i32, operation: &'static str) -> Result<TradeSide> {
    match trade_type {
        1 => Ok(TradeSide::Buy),
        2 => Ok(TradeSide::Sell),
        other => Err(error::invalid_field(
            operation,
            "tradeType",
            format!("unsupported MEXC public deal tradeType `{other}`"),
        )),
    }
}

fn parse_unix_millis_timestamp(
    timestamp_millis: i64,
    operation: &'static str,
) -> Result<OffsetDateTime> {
    common::parse_unix_millis_timestamp(timestamp_millis)
        .map_err(|err| error::invalid_field(operation, "time", err.to_string()))
}
