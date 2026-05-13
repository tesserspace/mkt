use async_trait::async_trait;
use mkt_types::{
    Balance, Fill, FuturesCancelOrderRequest, FuturesOrderQuery, FuturesOrderRequest, Kline,
    KlineRequest, LastPrice, MarketInfo, Order, OrderBook, Position, SetLeverageRequest,
    SpotCancelOrderRequest, SpotOrderQuery, SpotOrderRequest, Symbol, Trade,
};

use crate::{
    Capabilities, EventStream, PrivateEventStream, PrivateSubscription, Result, Subscription,
};
use mkt_types::ExchangeId;

pub trait ExchangeInfo: Send + Sync {
    fn id(&self) -> ExchangeId;
    fn capabilities(&self) -> Capabilities;
}

#[async_trait]
pub trait MarketData: Send + Sync {
    async fn markets(&self) -> Result<Vec<MarketInfo>>;
    async fn last_price(&self, symbol: &Symbol) -> Result<LastPrice>;
    async fn last_prices(&self, symbols: Option<&[Symbol]>) -> Result<Vec<LastPrice>>;
    async fn order_book(&self, symbol: &Symbol, depth: Option<u32>) -> Result<OrderBook>;
    async fn recent_trades(&self, symbol: &Symbol, limit: Option<u32>) -> Result<Vec<Trade>>;
    async fn klines(&self, request: KlineRequest) -> Result<Vec<Kline>>;
}

#[async_trait]
pub trait SpotTrading: Send + Sync {
    async fn place_spot_order(&self, request: SpotOrderRequest) -> Result<Order>;
    async fn cancel_spot_order(&self, request: SpotCancelOrderRequest) -> Result<Order>;
    async fn spot_order(&self, query: SpotOrderQuery) -> Result<Order>;
    async fn open_spot_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>>;
    async fn spot_fills(&self, query: SpotOrderQuery) -> Result<Vec<Fill>>;
}

#[async_trait]
pub trait FuturesTrading: Send + Sync {
    async fn place_futures_order(&self, request: FuturesOrderRequest) -> Result<Order>;
    async fn cancel_futures_order(&self, request: FuturesCancelOrderRequest) -> Result<Order>;
    async fn futures_order(&self, query: FuturesOrderQuery) -> Result<Order>;
    async fn open_futures_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>>;
    async fn futures_fills(&self, query: FuturesOrderQuery) -> Result<Vec<Fill>>;
    async fn positions(&self, symbol: Option<&Symbol>) -> Result<Vec<Position>>;
    async fn set_leverage(&self, request: SetLeverageRequest) -> Result<()>;
}

#[async_trait]
pub trait Account: Send + Sync {
    async fn balances(&self) -> Result<Vec<Balance>>;
}

#[async_trait]
pub trait PublicStream: Send + Sync {
    async fn subscribe_public(
        &self,
        subscriptions: Vec<Subscription>,
    ) -> Result<Box<dyn EventStream>>;
}

#[async_trait]
pub trait PrivateStream: Send + Sync {
    async fn subscribe_private(
        &self,
        subscriptions: Vec<PrivateSubscription>,
    ) -> Result<Box<dyn PrivateEventStream>>;
}
