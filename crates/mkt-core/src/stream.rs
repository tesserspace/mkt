use std::time::Duration;

use async_trait::async_trait;
use mkt_types::{
    AggTrade, AveragePrice, Balance, BlockTrade, BookTicker, ExchangeId, Fill, Kline, KlineRequest,
    LastPrice, MiniTicker, Order, OrderBook, OrderBookDelta, Position, Symbol, Trade,
};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Subscription {
    LastPrice(Symbol),
    OrderBook {
        symbol: Symbol,
        depth: Option<u16>,
    },
    OrderBookDeltas {
        symbol: Symbol,
        max_update_interval: Option<Duration>,
    },
    Trades(Symbol),
    AggTrades(Symbol),
    BlockTrades(Symbol),
    BookTicker(Symbol),
    AveragePrice(Symbol),
    MiniTicker(Symbol),
    Klines(KlineRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrivateSubscription {
    Orders,
    Fills,
    Balances,
    Positions,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MarketDataEvent {
    LastPrice(LastPrice),
    OrderBook(OrderBook),
    OrderBookDelta(OrderBookDelta),
    Trade(Trade),
    AggTrade(AggTrade),
    BlockTrade(BlockTrade),
    BookTicker(BookTicker),
    AveragePrice(AveragePrice),
    MiniTicker(MiniTicker),
    Kline(Kline),
    Raw {
        exchange_id: ExchangeId,
        payload: RawPayload,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum RawPayload {
    Text(String),
    Binary(Vec<u8>),
}

#[async_trait]
pub trait EventStream: Send {
    async fn next(&mut self) -> Result<Option<MarketDataEvent>>;

    async fn close(&mut self) -> Result<()>;
}

#[async_trait]
pub trait PrivateEventStream: Send {
    async fn next(&mut self) -> Result<Option<PrivateEvent>>;

    async fn close(&mut self) -> Result<()>;
}
