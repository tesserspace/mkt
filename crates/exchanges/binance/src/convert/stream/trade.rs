use std::collections::BTreeSet;

use mkt_core::{MarketDataEvent, Result};
use mkt_types::{AggTrade, BlockTrade, LastPrice, Symbol, Trade, TradeSide};
use rust_decimal::Decimal;
use serde_json::Value;
use time::OffsetDateTime;

use super::{super::internal, TradeOutput};

pub(super) fn trade_events_from_value(
    value: &Value,
    outputs: &BTreeSet<TradeOutput>,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    let response: binance_sdk::spot::websocket_streams::TradeResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid trade payload: {err}"))
        })?;
    let trade = ParsedTrade::from_response(response, operation)?;
    let mut events = Vec::with_capacity(outputs.len());

    for output in outputs {
        match output {
            TradeOutput::LastPrice => {
                events.push(MarketDataEvent::LastPrice(trade.last_price()));
            }
            TradeOutput::Trade => {
                events.push(MarketDataEvent::Trade(trade.trade(operation)?));
            }
        }
    }

    Ok(events)
}

pub(super) fn agg_trade_from_value(value: &Value, operation: &'static str) -> Result<AggTrade> {
    let response: binance_sdk::spot::websocket_streams::AggTradeResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid aggregate trade payload: {err}"))
        })?;

    AggTrade::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .id(response.a.map(|value| value.to_string()))
        .price(internal::parse_required_decimal(
            response.p, operation, "p",
        )?)
        .quantity(internal::parse_required_decimal(
            response.q, operation, "q",
        )?)
        .side(trade_side_from_buyer_maker(response.m, operation, "m")?)
        .first_trade_id(response.f.map(|value| value.to_string()))
        .last_trade_id(response.l.map(|value| value.to_string()))
        .timestamp(internal::parse_required_unix_millis_timestamp(
            response.t_uppercase,
            operation,
            "T",
        )?)
        .event_time(internal::parse_optional_unix_millis_timestamp(
            response.e_uppercase,
            operation,
            "E",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "agg_trade", err.to_string()))
}

pub(super) fn block_trade_from_value(value: &Value, operation: &'static str) -> Result<BlockTrade> {
    let response: binance_sdk::spot::websocket_streams::BlockTradeResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid block trade payload: {err}"))
        })?;

    BlockTrade::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .id(response.t.map(|value| value.to_string()))
        .price(internal::parse_required_decimal(
            response.p, operation, "p",
        )?)
        .quantity(internal::parse_required_decimal(
            response.q, operation, "q",
        )?)
        .side(trade_side_from_buyer_maker(response.m, operation, "m")?)
        .timestamp(internal::parse_required_unix_millis_timestamp(
            response.t_uppercase,
            operation,
            "T",
        )?)
        .event_time(internal::parse_optional_unix_millis_timestamp(
            response.e_uppercase,
            operation,
            "E",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "block_trade", err.to_string()))
}

struct ParsedTrade {
    symbol: Symbol,
    id: Option<String>,
    price: Decimal,
    quantity: Decimal,
    side: TradeSide,
    timestamp: OffsetDateTime,
}

impl ParsedTrade {
    fn from_response(
        response: binance_sdk::spot::websocket_streams::TradeResponse,
        operation: &'static str,
    ) -> Result<Self> {
        Ok(Self {
            symbol: Symbol::spot(
                response
                    .s
                    .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
            ),
            id: response.t.map(|value| value.to_string()),
            price: internal::parse_required_decimal(response.p, operation, "p")?,
            quantity: internal::parse_required_decimal(response.q, operation, "q")?,
            side: trade_side_from_buyer_maker(response.m, operation, "m")?,
            timestamp: internal::parse_required_unix_millis_timestamp(
                response.t_uppercase,
                operation,
                "T",
            )?,
        })
    }

    fn last_price(&self) -> LastPrice {
        LastPrice::new(self.symbol.clone(), self.price)
    }

    fn trade(&self, operation: &'static str) -> Result<Trade> {
        Trade::builder()
            .symbol(self.symbol.clone())
            .id(self.id.clone())
            .price(self.price)
            .quantity(self.quantity)
            .side(self.side)
            .timestamp(self.timestamp)
            .build()
            .map_err(|err| crate::error::invalid_field(operation, "trade", err.to_string()))
    }
}

fn trade_side_from_buyer_maker(
    buyer_is_maker: Option<bool>,
    operation: &'static str,
    field: &'static str,
) -> Result<TradeSide> {
    match buyer_is_maker {
        Some(true) => Ok(TradeSide::Sell),
        Some(false) => Ok(TradeSide::Buy),
        None => Err(crate::error::missing_field(operation, field)),
    }
}
