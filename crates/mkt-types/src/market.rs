use derive_builder::Builder;
use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;

use crate::ExchangeId;
use strum_macros::{Display, EnumString};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarketFamily {
    Spot,
    Derivative,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettlementMode {
    Linear,
    Inverse,
}

impl fmt::Display for SettlementMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Linear => "linear",
            Self::Inverse => "inverse",
        })
    }
}

impl FromStr for SettlementMode {
    type Err = MarketKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "linear" => Ok(Self::Linear),
            "inverse" => Ok(Self::Inverse),
            _ => Err(MarketKindParseError::Invalid(value.to_owned())),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContractMaturity {
    Perpetual,
    Expiring,
}

impl fmt::Display for ContractMaturity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Perpetual => "perpetual",
            Self::Expiring => "expiring",
        })
    }
}

impl FromStr for ContractMaturity {
    type Err = MarketKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "perpetual" => Ok(Self::Perpetual),
            "expiring" | "future" => Ok(Self::Expiring),
            _ => Err(MarketKindParseError::Invalid(value.to_owned())),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DerivativeKind {
    pub maturity: ContractMaturity,
    pub settlement: SettlementMode,
}

impl DerivativeKind {
    pub const fn new(maturity: ContractMaturity, settlement: SettlementMode) -> Self {
        Self {
            maturity,
            settlement,
        }
    }

    pub const fn perpetual(settlement: SettlementMode) -> Self {
        Self::new(ContractMaturity::Perpetual, settlement)
    }

    pub const fn expiring(settlement: SettlementMode) -> Self {
        Self::new(ContractMaturity::Expiring, settlement)
    }
}

impl fmt::Display for DerivativeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.settlement, self.maturity)
    }
}

impl FromStr for DerivativeKind {
    type Err = MarketKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (settlement, maturity) = value
            .split_once('_')
            .ok_or_else(|| MarketKindParseError::Invalid(value.to_owned()))?;

        Ok(Self::new(maturity.parse()?, settlement.parse()?))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarketKind {
    Spot,
    Derivative(DerivativeKind),
}

impl MarketKind {
    pub const fn spot() -> Self {
        Self::Spot
    }

    pub const fn derivative(kind: DerivativeKind) -> Self {
        Self::Derivative(kind)
    }

    pub const fn linear_perpetual() -> Self {
        Self::Derivative(DerivativeKind::perpetual(SettlementMode::Linear))
    }

    pub const fn inverse_perpetual() -> Self {
        Self::Derivative(DerivativeKind::perpetual(SettlementMode::Inverse))
    }

    pub const fn linear_expiring() -> Self {
        Self::Derivative(DerivativeKind::expiring(SettlementMode::Linear))
    }

    pub const fn inverse_expiring() -> Self {
        Self::Derivative(DerivativeKind::expiring(SettlementMode::Inverse))
    }

    pub fn family(self) -> MarketFamily {
        match self {
            Self::Spot => MarketFamily::Spot,
            Self::Derivative(_) => MarketFamily::Derivative,
        }
    }

    pub fn is_derivative(self) -> bool {
        matches!(self, Self::Derivative(_))
    }

    pub fn derivative_kind(self) -> Option<DerivativeKind> {
        match self {
            Self::Spot => None,
            Self::Derivative(kind) => Some(kind),
        }
    }
}

impl fmt::Display for MarketKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spot => f.write_str("spot"),
            Self::Derivative(kind) => write!(f, "{kind}"),
        }
    }
}

impl FromStr for MarketKind {
    type Err = MarketKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "spot" => Ok(Self::Spot),
            other => Ok(Self::Derivative(other.parse()?)),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for MarketKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for MarketKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <String as Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketKindParseError {
    Invalid(String),
}

impl fmt::Display for MarketKindParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(value) => write!(f, "invalid market kind: {value}"),
        }
    }
}

impl std::error::Error for MarketKindParseError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub kind: MarketKind,
    pub venue_symbol: String,
}

impl Symbol {
    pub fn spot(venue_symbol: impl Into<String>) -> Self {
        Self {
            kind: MarketKind::Spot,
            venue_symbol: venue_symbol.into(),
        }
    }

    pub fn derivative(kind: DerivativeKind, venue_symbol: impl Into<String>) -> Self {
        Self {
            kind: MarketKind::Derivative(kind),
            venue_symbol: venue_symbol.into(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum MarketStatus {
    Trading,
    Halted,
    PreLaunch,
    Delisted,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct PriceFilter {
    #[builder(default)]
    pub min_price: Option<Decimal>,
    #[builder(default)]
    pub max_price: Option<Decimal>,
    #[builder(default)]
    pub tick_size: Option<Decimal>,
}

impl PriceFilter {
    pub fn builder() -> PriceFilterBuilder {
        PriceFilterBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct LotSizeFilter {
    #[builder(default)]
    pub min_quantity: Option<Decimal>,
    #[builder(default)]
    pub max_quantity: Option<Decimal>,
    #[builder(default)]
    pub step_size: Option<Decimal>,
}

impl LotSizeFilter {
    pub fn builder() -> LotSizeFilterBuilder {
        LotSizeFilterBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct NotionalConstraints {
    #[builder(default)]
    pub min_notional: Option<Decimal>,
    #[builder(default)]
    pub max_notional: Option<Decimal>,
    #[builder(default)]
    pub apply_min_to_market: Option<bool>,
    #[builder(default)]
    pub apply_max_to_market: Option<bool>,
}

impl NotionalConstraints {
    pub fn builder() -> NotionalConstraintsBuilder {
        NotionalConstraintsBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarketQuantityMode {
    Base,
    Quote,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct QuantityModeSupport {
    pub mode: MarketQuantityMode,
    #[builder(default)]
    pub order_types: Vec<crate::OrderType>,
    #[builder(default)]
    pub sides: Vec<crate::OrderSide>,
}

impl QuantityModeSupport {
    pub fn builder() -> QuantityModeSupportBuilder {
        QuantityModeSupportBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder, Default)]
#[builder(pattern = "owned", setter(into))]
pub struct TradingPermissions {
    #[builder(default)]
    pub spot_order_entry_allowed: Option<bool>,
    #[builder(default)]
    pub supported_order_types: Vec<crate::OrderType>,
    #[builder(default)]
    pub quantity_mode_support: Vec<QuantityModeSupport>,
}

impl TradingPermissions {
    pub fn builder() -> TradingPermissionsBuilder {
        TradingPermissionsBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder, Default)]
#[builder(pattern = "owned", setter(into))]
pub struct TradingConstraints {
    #[builder(default)]
    pub price_filter: Option<PriceFilter>,
    #[builder(default)]
    pub lot_size: Option<LotSizeFilter>,
    #[builder(default)]
    pub market_lot_size: Option<LotSizeFilter>,
    #[builder(default)]
    pub notional: Option<NotionalConstraints>,
}

impl TradingConstraints {
    pub fn builder() -> TradingConstraintsBuilder {
        TradingConstraintsBuilder::default()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct MarketInfo {
    /// Canonical exchange identifier for the venue that exposed this market.
    pub exchange_id: ExchangeId,
    /// Unified symbol descriptor, including market kind and venue-specific symbol string.
    pub symbol: Symbol,
    /// Current lifecycle status reported by the exchange.
    pub status: MarketStatus,
    /// Venue-reported base asset code for this market.
    pub base_asset: String,
    /// Venue-reported quote asset code for this market.
    pub quote_asset: String,
    #[builder(default)]
    /// Exchange-reported precision for base-asset quantities at the symbol level, when available.
    ///
    /// This is raw venue metadata. It may differ from executable step-size filters and should not
    /// be treated as a substitute for lot-size validation.
    pub base_asset_precision: Option<i64>,
    #[builder(default)]
    /// Exchange-reported precision for quote-denominated values at the symbol level, when
    /// available.
    ///
    /// On venues such as Binance spot, this is the most relevant symbol-level precision hint for
    /// quote-sized order entry like `quoteOrderQty`.
    pub quote_precision: Option<i64>,
    #[builder(default)]
    /// Exchange-reported precision for the quote asset at the symbol level, when available.
    ///
    /// Some venues expose both `quote_precision` and `quote_asset_precision`. When they differ,
    /// adapter code should prefer the field that the venue documents for order-entry precision and
    /// treat this field as auxiliary metadata or fallback.
    pub quote_asset_precision: Option<i64>,
    #[builder(default)]
    /// Venue capabilities describing which order types, sides, and quantity modes are supported.
    pub trading_permissions: TradingPermissions,
    #[builder(default)]
    /// Venue execution constraints such as price filters, lot sizes, and notional limits.
    pub trading_constraints: TradingConstraints,
}

impl MarketInfo {
    pub fn builder() -> MarketInfoBuilder {
        MarketInfoBuilder::default()
    }
}
