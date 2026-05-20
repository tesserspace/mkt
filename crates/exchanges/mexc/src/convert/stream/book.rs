use mkt_core::Result;
use mkt_exchange_common as common;
use mkt_types::{BookTicker, OrderBook, OrderBookDelta, OrderBookLevel, Symbol};
use rust_decimal::Decimal;

use crate::{
    error,
    protobuf::{
        PublicAggreBookTickerV3Api, PublicAggreDepthV3ApiItem, PublicAggreDepthsV3Api,
        PublicIncreaseDepthV3ApiItem, PublicIncreaseDepthsV3Api, PublicLimitDepthV3ApiItem,
        PublicLimitDepthsV3Api,
    },
};

pub(crate) fn book_ticker_from_aggre_book_ticker(
    symbol: Symbol,
    payload: PublicAggreBookTickerV3Api,
    operation: &'static str,
) -> Result<BookTicker> {
    book_ticker_from_fields(
        symbol,
        BookTickerFields::new(
            payload.bid_price,
            payload.bid_quantity,
            payload.ask_price,
            payload.ask_quantity,
        ),
        operation,
    )
}

pub(crate) fn order_book_from_limit_depths(
    symbol: Symbol,
    payload: PublicLimitDepthsV3Api,
    operation: &'static str,
) -> Result<OrderBook> {
    OrderBook::builder()
        .symbol(symbol)
        .bids(levels_from_limit_items(payload.bids, operation, "bids")?)
        .asks(levels_from_limit_items(payload.asks, operation, "asks")?)
        .last_update_id(Some(payload.version))
        .timestamp(None)
        .build()
        .map_err(|err| error::invalid_field(operation, "order_book", err.to_string()))
}

pub(crate) fn order_book_delta_from_increase_depths(
    symbol: Symbol,
    payload: PublicIncreaseDepthsV3Api,
    operation: &'static str,
) -> Result<OrderBookDelta> {
    OrderBookDelta::builder()
        .symbol(symbol)
        .last_update_id(Some(payload.version))
        .bids(levels_from_increase_items(payload.bids, operation, "bids")?)
        .asks(levels_from_increase_items(payload.asks, operation, "asks")?)
        .timestamp(None)
        .build()
        .map_err(|err| error::invalid_field(operation, "order_book_delta", err.to_string()))
}

pub(crate) fn order_book_delta_from_aggre_depths(
    symbol: Symbol,
    payload: PublicAggreDepthsV3Api,
    operation: &'static str,
) -> Result<OrderBookDelta> {
    OrderBookDelta::builder()
        .symbol(symbol)
        .first_update_id(Some(payload.from_version))
        .last_update_id(Some(payload.to_version))
        .bids(levels_from_aggre_items(payload.bids, operation, "bids")?)
        .asks(levels_from_aggre_items(payload.asks, operation, "asks")?)
        .timestamp(None)
        .build()
        .map_err(|err| error::invalid_field(operation, "order_book_delta", err.to_string()))
}

fn levels_from_limit_items(
    items: Vec<PublicLimitDepthV3ApiItem>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    items
        .into_iter()
        .map(|level| level_from_strings(level.price, level.quantity, operation, field))
        .collect()
}

fn levels_from_increase_items(
    items: Vec<PublicIncreaseDepthV3ApiItem>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    items
        .into_iter()
        .map(|level| level_from_strings(level.price, level.quantity, operation, field))
        .collect()
}

fn levels_from_aggre_items(
    items: Vec<PublicAggreDepthV3ApiItem>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    items
        .into_iter()
        .map(|level| level_from_strings(level.price, level.quantity, operation, field))
        .collect()
}

fn level_from_strings(
    price: String,
    quantity: String,
    operation: &'static str,
    field: &'static str,
) -> Result<OrderBookLevel> {
    Ok(OrderBookLevel::new(
        parse_decimal(price, operation, field)?,
        parse_decimal(quantity, operation, field)?,
    ))
}

struct BookTickerFields {
    bid_price: String,
    bid_quantity: String,
    ask_price: String,
    ask_quantity: String,
}

impl BookTickerFields {
    fn new(
        bid_price: String,
        bid_quantity: String,
        ask_price: String,
        ask_quantity: String,
    ) -> Self {
        Self {
            bid_price,
            bid_quantity,
            ask_price,
            ask_quantity,
        }
    }
}

fn book_ticker_from_fields(
    symbol: Symbol,
    fields: BookTickerFields,
    operation: &'static str,
) -> Result<BookTicker> {
    BookTicker::builder()
        .symbol(symbol)
        .bid_price(parse_decimal(fields.bid_price, operation, "bidPrice")?)
        .bid_quantity(parse_decimal(
            fields.bid_quantity,
            operation,
            "bidQuantity",
        )?)
        .ask_price(parse_decimal(fields.ask_price, operation, "askPrice")?)
        .ask_quantity(parse_decimal(
            fields.ask_quantity,
            operation,
            "askQuantity",
        )?)
        .timestamp(None)
        .build()
        .map_err(|err| error::invalid_field(operation, "book_ticker", err.to_string()))
}

fn parse_decimal(raw: String, operation: &'static str, field: &'static str) -> Result<Decimal> {
    common::parse_decimal(raw.as_str())
        .map_err(|err| error::invalid_field(operation, field, err.to_string()))
}
