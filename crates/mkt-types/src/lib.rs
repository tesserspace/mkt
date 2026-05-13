//! Stable business data types shared by mkt clients and strategy code.

mod account;
mod exchange;
mod extensions;
mod market;
mod market_data;
mod trading;

pub use account::{Balance, Position};
pub use exchange::{ExchangeId, ExchangeIdParseError, ExtensionExchangeId, KnownExchange};
pub use extensions::{ExtensionValueError, Extensions, NamespaceKey, NamespaceKeyError};
pub use market::{
    ContractMaturity, DerivativeKind, LotSizeFilter, MarketFamily, MarketInfo, MarketKind,
    MarketKindParseError, MarketQuantityMode, MarketStatus, NotionalConstraints, PriceFilter,
    QuantityModeSupport, SettlementMode, Symbol, TradingConstraints, TradingPermissions,
};
pub use market_data::{
    Kline, KlineInterval, KlineRequest, LastPrice, OrderBook, OrderBookLevel, Trade, TradeSide,
};
pub use rust_decimal::Decimal;
pub use trading::{
    ClientOrderId, Fill, FuturesCancelOrderRequest, FuturesOrderQuery, FuturesOrderRequest,
    MarginMode, Order, OrderId, OrderKey, OrderQuantity, OrderSide, OrderStatus, OrderType,
    PositionSide, SetLeverageRequest, SpotCancelOrderRequest, SpotOrderQuery, SpotOrderRequest,
    TimeInForce,
};
