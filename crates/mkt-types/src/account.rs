use derive_builder::Builder;
use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::{Extensions, MarginMode, PositionSide, Symbol};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Balance {
    pub asset: String,
    pub available: Decimal,
    pub locked: Decimal,
    pub total: Decimal,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub timestamp: OffsetDateTime,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Balance {
    pub fn builder() -> BalanceBuilder {
        BalanceBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct Position {
    #[builder(default)]
    pub position_id: Option<String>,
    pub symbol: Symbol,
    pub side: PositionSide,
    #[builder(default)]
    pub margin_mode: Option<MarginMode>,
    pub quantity: Decimal,
    #[builder(default)]
    pub entry_price: Option<Decimal>,
    #[builder(default)]
    pub mark_price: Option<Decimal>,
    #[builder(default)]
    pub unrealized_pnl: Option<Decimal>,
    #[builder(default)]
    pub leverage: Option<Decimal>,
    #[builder(default)]
    pub liquidation_price: Option<Decimal>,
    #[cfg_attr(
        feature = "serde",
        serde(with = "time::serde::timestamp::milliseconds")
    )]
    pub updated_at: OffsetDateTime,
    #[builder(default)]
    pub extensions: Extensions,
}

impl Position {
    pub fn builder() -> PositionBuilder {
        PositionBuilder::default()
    }
}
