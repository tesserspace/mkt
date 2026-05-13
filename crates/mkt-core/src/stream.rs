use async_trait::async_trait;
use mkt_types::{
    Balance, ExchangeId, Fill, Kline, KlineRequest, LastPrice, Order, OrderBook, Position, Symbol,
    Trade,
};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subscription {
    LastPrice(Symbol),
    OrderBook { symbol: Symbol, depth: Option<u32> },
    Trades(Symbol),
    Klines(KlineRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateSubscription {
    Orders,
    Fills,
    Balances,
    Positions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketDataEvent {
    LastPrice(LastPrice),
    OrderBook(OrderBook),
    Trade(Trade),
    Kline(Kline),
    Raw {
        exchange_id: ExchangeId,
        payload: RawPayload,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrivateEvent {
    Order(Order),
    Fill(Fill),
    Balance(Balance),
    Position(Position),
    Raw {
        exchange_id: ExchangeId,
        payload: RawPayload,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawPayload {
    Text(String),
    Binary(Vec<u8>),
}

#[async_trait]
pub trait EventStream: Send {
    async fn next(&mut self) -> Result<Option<MarketDataEvent>>;
}

#[async_trait]
pub trait PrivateEventStream: Send {
    async fn next(&mut self) -> Result<Option<PrivateEvent>>;
}
