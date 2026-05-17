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

    fn supports_order_type(&self, order_type: crate::OrderType) -> bool {
        self.order_types.is_empty() || self.order_types.contains(&order_type)
    }

    fn supports_side(&self, side: crate::OrderSide) -> bool {
        self.sides.is_empty() || self.sides.contains(&side)
    }

    fn supports(&self, order_type: crate::OrderType, side: crate::OrderSide) -> bool {
        self.supports_order_type(order_type) && self.supports_side(side)
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

    pub fn allows_spot_order_entry(&self) -> bool {
        self.spot_order_entry_allowed.unwrap_or(true)
    }

    pub fn supports_order_type(&self, order_type: crate::OrderType) -> bool {
        self.supported_order_types.contains(&order_type)
    }

    pub fn supports_quantity_mode(
        &self,
        mode: MarketQuantityMode,
        order_type: crate::OrderType,
        side: crate::OrderSide,
    ) -> bool {
        self.quantity_mode_support
            .iter()
            .filter(|support| support.mode == mode)
            .any(|support| support.supports(order_type, side))
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

    fn lot_size_for(&self, order_type: crate::OrderType) -> Option<&LotSizeFilter> {
        match order_type {
            crate::OrderType::Market | crate::OrderType::StopMarket => {
                self.market_lot_size.as_ref().or(self.lot_size.as_ref())
            }
            _ => self.lot_size.as_ref().or(self.market_lot_size.as_ref()),
        }
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

    pub fn is_trading(&self) -> bool {
        matches!(self.status, MarketStatus::Trading)
    }

    pub fn allows_spot_order_entry(&self) -> bool {
        self.trading_permissions.allows_spot_order_entry()
    }

    pub fn tick_size(&self) -> Option<Decimal> {
        self.trading_constraints
            .price_filter
            .as_ref()
            .and_then(|filter| filter.tick_size)
            .filter(|value| *value > Decimal::ZERO)
    }

    pub fn quote_scale(&self) -> Option<u32> {
        self.quote_precision
            .and_then(|scale| u32::try_from(scale).ok())
            .or_else(|| {
                self.quote_asset_precision
                    .and_then(|scale| u32::try_from(scale).ok())
            })
    }

    pub fn min_notional_or(&self, fallback: Decimal) -> Decimal {
        self.trading_constraints
            .notional
            .as_ref()
            .and_then(|constraints| constraints.min_notional)
            .filter(|value| *value > Decimal::ZERO)
            .unwrap_or(fallback)
            .max(fallback)
    }

    pub fn lot_size_for(&self, order_type: crate::OrderType) -> Option<&LotSizeFilter> {
        self.trading_constraints.lot_size_for(order_type)
    }

    pub fn min_quantity(&self, order_type: crate::OrderType) -> Option<Decimal> {
        self.positive_lot_size_value(order_type, |filter| filter.min_quantity)
    }

    pub fn max_quantity(&self, order_type: crate::OrderType) -> Option<Decimal> {
        self.positive_lot_size_value(order_type, |filter| filter.max_quantity)
    }

    pub fn step_size(&self, order_type: crate::OrderType) -> Option<Decimal> {
        self.positive_lot_size_value(order_type, |filter| filter.step_size)
    }

    fn positive_lot_size_value(
        &self,
        order_type: crate::OrderType,
        value: impl Fn(&LotSizeFilter) -> Option<Decimal>,
    ) -> Option<Decimal> {
        let trading_constraints = &self.trading_constraints;
        let (primary, fallback) = match order_type {
            crate::OrderType::Market | crate::OrderType::StopMarket => (
                trading_constraints.market_lot_size.as_ref(),
                trading_constraints.lot_size.as_ref(),
            ),
            _ => (
                trading_constraints.lot_size.as_ref(),
                trading_constraints.market_lot_size.as_ref(),
            ),
        };

        primary
            .and_then(&value)
            .filter(|current| *current > Decimal::ZERO)
            .or_else(|| {
                fallback
                    .and_then(value)
                    .filter(|current| *current > Decimal::ZERO)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LotSizeFilter, MarketInfo, MarketQuantityMode, MarketStatus, NotionalConstraints,
        PriceFilter, QuantityModeSupport, TradingConstraints, TradingPermissions,
    };
    use crate::{ExchangeId, KnownExchange, OrderSide, OrderType, Symbol};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("test decimal must be valid")
    }

    fn lot_size(min_quantity: &str, max_quantity: &str, step_size: &str) -> LotSizeFilter {
        LotSizeFilter::builder()
            .min_quantity(decimal(min_quantity))
            .max_quantity(decimal(max_quantity))
            .step_size(decimal(step_size))
            .build()
            .expect("lot size must build")
    }

    fn market_info(
        status: MarketStatus,
        trading_permissions: TradingPermissions,
        trading_constraints: TradingConstraints,
    ) -> MarketInfo {
        MarketInfo::builder()
            .exchange_id(ExchangeId::from(KnownExchange::Binance))
            .symbol(Symbol::spot("BTCUSDT"))
            .status(status)
            .base_asset("BTC")
            .quote_asset("USDT")
            .quote_precision(8)
            .quote_asset_precision(6)
            .trading_permissions(trading_permissions)
            .trading_constraints(trading_constraints)
            .build()
            .expect("market info must build")
    }

    #[test]
    fn trading_permissions_helpers_use_expected_defaults() {
        let unknown = TradingPermissions::default();
        assert!(unknown.allows_spot_order_entry());
        assert!(!unknown.supports_order_type(OrderType::Limit));
        assert!(!unknown.supports_quantity_mode(
            MarketQuantityMode::Base,
            OrderType::Limit,
            OrderSide::Buy
        ));

        let permissions = TradingPermissions::builder()
            .spot_order_entry_allowed(Some(false))
            .supported_order_types([OrderType::Limit, OrderType::Market])
            .quantity_mode_support(vec![
                QuantityModeSupport::builder()
                    .mode(MarketQuantityMode::Base)
                    .order_types([OrderType::Limit])
                    .sides([OrderSide::Buy, OrderSide::Sell])
                    .build()
                    .expect("base support must build"),
                QuantityModeSupport::builder()
                    .mode(MarketQuantityMode::Quote)
                    .order_types([OrderType::Market])
                    .sides([OrderSide::Buy])
                    .build()
                    .expect("quote support must build"),
            ])
            .build()
            .expect("permissions must build");

        assert!(!permissions.allows_spot_order_entry());
        assert!(permissions.supports_order_type(OrderType::Limit));
        assert!(!permissions.supports_order_type(OrderType::PostOnly));
        assert!(permissions.supports_quantity_mode(
            MarketQuantityMode::Base,
            OrderType::Limit,
            OrderSide::Buy
        ));
        assert!(!permissions.supports_quantity_mode(
            MarketQuantityMode::Base,
            OrderType::Market,
            OrderSide::Buy
        ));
        assert!(permissions.supports_quantity_mode(
            MarketQuantityMode::Quote,
            OrderType::Market,
            OrderSide::Buy
        ));
        assert!(!permissions.supports_quantity_mode(
            MarketQuantityMode::Quote,
            OrderType::Market,
            OrderSide::Sell
        ));
    }

    #[test]
    fn market_info_helpers_pick_effective_constraints() {
        let limit_lot = lot_size("0.01", "50", "0.01");
        let market_lot = lot_size("0.2", "2", "0.2");
        let market = market_info(
            MarketStatus::Trading,
            TradingPermissions::default(),
            TradingConstraints::builder()
                .price_filter(
                    PriceFilter::builder()
                        .tick_size(decimal("0.25"))
                        .build()
                        .expect("price filter must build"),
                )
                .lot_size(limit_lot.clone())
                .market_lot_size(market_lot.clone())
                .notional(
                    NotionalConstraints::builder()
                        .min_notional(decimal("12"))
                        .build()
                        .expect("notional must build"),
                )
                .build()
                .expect("constraints must build"),
        );

        assert!(market.is_trading());
        assert!(market.allows_spot_order_entry());
        assert_eq!(market.tick_size(), Some(decimal("0.25")));
        assert_eq!(market.quote_scale(), Some(8));
        assert_eq!(market.min_notional_or(decimal("10")), decimal("12"));
        assert_eq!(market.step_size(OrderType::Limit), Some(decimal("0.01")));
        assert_eq!(market.step_size(OrderType::Market), Some(decimal("0.2")));
        assert_eq!(market.min_quantity(OrderType::Limit), Some(decimal("0.01")));
        assert_eq!(market.min_quantity(OrderType::Market), Some(decimal("0.2")));
        assert_eq!(market.max_quantity(OrderType::Market), Some(decimal("2")));
        assert_eq!(market.lot_size_for(OrderType::Limit), Some(&limit_lot));
        assert_eq!(
            market.lot_size_for(OrderType::StopMarket),
            Some(&market_lot)
        );
    }

    #[test]
    fn market_info_helpers_filter_non_positive_values_and_fallback() {
        let zero_lot = lot_size("0", "0", "0");
        let market = market_info(
            MarketStatus::Halted,
            TradingPermissions::builder()
                .spot_order_entry_allowed(Some(true))
                .build()
                .expect("permissions must build"),
            TradingConstraints::builder()
                .price_filter(
                    PriceFilter::builder()
                        .tick_size(decimal("0"))
                        .build()
                        .expect("price filter must build"),
                )
                .lot_size(zero_lot)
                .notional(
                    NotionalConstraints::builder()
                        .min_notional(decimal("0"))
                        .build()
                        .expect("notional must build"),
                )
                .build()
                .expect("constraints must build"),
        );

        assert!(!market.is_trading());
        assert!(market.allows_spot_order_entry());
        assert_eq!(market.tick_size(), None);
        assert_eq!(market.min_quantity(OrderType::Limit), None);
        assert_eq!(market.max_quantity(OrderType::Market), None);
        assert_eq!(market.step_size(OrderType::Market), None);
        assert_eq!(market.min_notional_or(decimal("10")), decimal("10"));
    }

    #[test]
    fn market_order_constraints_fall_back_per_field() {
        let limit_lot = lot_size("0.01", "100", "0.01");
        let market_lot = lot_size("0", "50", "0");
        let market = market_info(
            MarketStatus::Trading,
            TradingPermissions::default(),
            TradingConstraints::builder()
                .lot_size(limit_lot)
                .market_lot_size(market_lot)
                .build()
                .expect("constraints must build"),
        );

        assert_eq!(
            market.min_quantity(OrderType::Market),
            Some(decimal("0.01"))
        );
        assert_eq!(market.max_quantity(OrderType::Market), Some(decimal("50")));
        assert_eq!(market.step_size(OrderType::Market), Some(decimal("0.01")));
        assert_eq!(
            market.min_quantity(OrderType::StopMarket),
            Some(decimal("0.01"))
        );
        assert_eq!(
            market.max_quantity(OrderType::StopMarket),
            Some(decimal("50"))
        );
        assert_eq!(
            market.step_size(OrderType::StopMarket),
            Some(decimal("0.01"))
        );
    }
}
