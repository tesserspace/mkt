use derive_builder::Builder;
use rust_decimal::Decimal;
use strum_macros::{Display, EnumString};
use time::OffsetDateTime;

use crate::{Extensions, MarketKind, Symbol};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub String);

impl OrderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientOrderId(pub String);

impl ClientOrderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrderKey {
    Exchange(OrderId),
    Client(ClientOrderId),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    PostOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE", ascii_case_insensitive)]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
    Gtx,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum PositionSide {
    Long,
    Short,
    Both,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum MarginMode {
    Cross,
    Isolated,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct SpotOrderRequest {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    #[builder(default)]
    pub price: Option<Decimal>,
    #[builder(default)]
    pub time_in_force: Option<TimeInForce>,
    #[builder(default)]
    pub client_order_id: Option<ClientOrderId>,
    #[builder(default)]
    pub extensions: Extensions,
}

impl SpotOrderRequest {
    pub fn builder() -> SpotOrderRequestBuilder {
        SpotOrderRequestBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct FuturesOrderRequest {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    #[builder(default)]
    pub price: Option<Decimal>,
    #[builder(default)]
    pub time_in_force: Option<TimeInForce>,
    #[builder(default)]
    pub position_side: Option<PositionSide>,
    #[builder(default)]
    pub reduce_only: bool,
    #[builder(default)]
    pub client_order_id: Option<ClientOrderId>,
    #[builder(default)]
    pub extensions: Extensions,
}

impl FuturesOrderRequest {
    pub fn builder() -> FuturesOrderRequestBuilder {
        FuturesOrderRequestBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotCancelOrderRequest {
    pub symbol: Symbol,
    pub key: OrderKey,
}

impl SpotCancelOrderRequest {
    pub fn new(symbol: Symbol, key: OrderKey) -> Self {
        Self { symbol, key }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesCancelOrderRequest {
    pub symbol: Symbol,
    pub key: OrderKey,
}

impl FuturesCancelOrderRequest {
    pub fn new(symbol: Symbol, key: OrderKey) -> Self {
        Self { symbol, key }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotOrderQuery {
    pub symbol: Symbol,
    pub key: OrderKey,
}

impl SpotOrderQuery {
    pub fn new(symbol: Symbol, key: OrderKey) -> Self {
        Self { symbol, key }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesOrderQuery {
    pub symbol: Symbol,
    pub key: OrderKey,
}

impl FuturesOrderQuery {
    pub fn new(symbol: Symbol, key: OrderKey) -> Self {
        Self { symbol, key }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Order {
    pub id: OrderId,
    #[builder(default)]
    pub client_order_id: Option<ClientOrderId>,
    pub symbol: Symbol,
    pub market_kind: MarketKind,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub status: OrderStatus,
    #[builder(default)]
    pub time_in_force: Option<TimeInForce>,
    #[builder(default)]
    pub price: Option<Decimal>,
    #[builder(default)]
    pub average_price: Option<Decimal>,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    #[builder(default)]
    pub original_quote_quantity: Option<Decimal>,
    #[builder(default)]
    pub cumulative_quote_quantity: Option<Decimal>,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub created_at: OffsetDateTime,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds::option")
    )]
    #[builder(default)]
    pub updated_at: Option<OffsetDateTime>,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Order {
    pub fn builder() -> OrderBuilder {
        OrderBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Fill {
    #[builder(default)]
    pub id: Option<String>,
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    #[builder(default)]
    pub fee: Option<Decimal>,
    #[builder(default)]
    pub fee_asset: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub timestamp: OffsetDateTime,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Fill {
    pub fn builder() -> FillBuilder {
        FillBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct SetLeverageRequest {
    pub symbol: Symbol,
    pub leverage: Decimal,
    #[builder(default)]
    pub margin_mode: Option<MarginMode>,
    #[builder(default)]
    pub position_side: Option<PositionSide>,
}

impl SetLeverageRequest {
    pub fn builder() -> SetLeverageRequestBuilder {
        SetLeverageRequestBuilder::default()
    }
}
