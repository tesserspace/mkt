//! Facade crate for mkt users.

pub use mkt_core as core;
pub use mkt_types as types;

pub mod prelude {
    pub use mkt_core::{
        Account, ExchangeHandle, ExchangeInfo, FuturesTrading, MarketData, PrivateStream,
        PublicStream, SpotTrading,
    };
    pub use mkt_types::{
        Balance, Decimal, ExchangeId, FuturesOrderRequest, Kline, KlineInterval, KnownExchange,
        LastPrice, MarketInfo, MarketKind, Order, OrderBook, OrderQuantity, Position,
        QuantityModeSupport, SpotOrderRequest, Symbol, TradingConstraints, TradingPermissions,
    };
}

#[cfg(any(
    feature = "binance",
    feature = "bitget",
    feature = "bybit",
    feature = "mexc",
    feature = "okx"
))]
pub mod exchanges {
    #[cfg(feature = "binance")]
    pub use mkt_exchange_binance as binance;
    #[cfg(feature = "bitget")]
    pub use mkt_exchange_bitget as bitget;
    #[cfg(feature = "bybit")]
    pub use mkt_exchange_bybit as bybit;
    #[cfg(feature = "mexc")]
    pub use mkt_exchange_mexc as mexc;
    #[cfg(feature = "okx")]
    pub use mkt_exchange_okx as okx;
}
