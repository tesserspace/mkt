use mkt_types::{
    Decimal, ExchangeId, KnownExchange, LotSizeFilter, MarketInfo, MarketQuantityMode,
    MarketStatus, NotionalConstraints, OrderSide, OrderType, PriceFilter, QuantityModeSupport,
    Symbol, TradingConstraints, TradingPermissions,
};
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
    quote_precision: Option<i64>,
    quote_asset_precision: Option<i64>,
    trading_permissions: TradingPermissions,
    trading_constraints: TradingConstraints,
) -> MarketInfo {
    MarketInfo::builder()
        .exchange_id(ExchangeId::from(KnownExchange::Binance))
        .symbol(Symbol::spot("BTCUSDT"))
        .status(status)
        .base_asset("BTC")
        .quote_asset("USDT")
        .quote_precision(quote_precision)
        .quote_asset_precision(quote_asset_precision)
        .trading_permissions(trading_permissions)
        .trading_constraints(trading_constraints)
        .build()
        .expect("market info must build")
}

#[test]
fn trading_permission_helpers_preserve_expected_defaults_and_support() {
    let unknown = TradingPermissions::default();

    assert!(unknown.allows_spot_order_entry());
    assert!(!unknown.supports_order_type(OrderType::Limit));
    assert!(!unknown.supports_quantity_mode(
        MarketQuantityMode::Base,
        OrderType::Limit,
        OrderSide::Buy
    ));

    let permissions = TradingPermissions::builder()
        .supported_order_types([OrderType::Limit, OrderType::Market])
        .quantity_mode_support([
            QuantityModeSupport::builder()
                .mode(MarketQuantityMode::Base)
                .order_types([OrderType::Limit, OrderType::Market])
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

    assert!(permissions.supports_order_type(OrderType::Limit));
    assert!(!permissions.supports_order_type(OrderType::StopLimit));
    assert!(permissions.supports_quantity_mode(
        MarketQuantityMode::Base,
        OrderType::Limit,
        OrderSide::Sell
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
    assert!(!permissions.supports_quantity_mode(
        MarketQuantityMode::Quote,
        OrderType::Limit,
        OrderSide::Buy
    ));
}

#[test]
fn market_info_helpers_expose_effective_constraints() {
    let limit_lot_size = lot_size("0.001", "100", "0.001");
    let market_lot_size = lot_size("0.01", "50", "0.01");
    let trading_constraints = TradingConstraints::builder()
        .price_filter(
            PriceFilter::builder()
                .tick_size(decimal("0.05"))
                .build()
                .expect("price filter must build"),
        )
        .lot_size(limit_lot_size.clone())
        .market_lot_size(market_lot_size.clone())
        .notional(
            NotionalConstraints::builder()
                .min_notional(decimal("10"))
                .build()
                .expect("notional must build"),
        )
        .build()
        .expect("constraints must build");
    let market = market_info(
        MarketStatus::Trading,
        Some(-1),
        Some(6),
        TradingPermissions::builder()
            .spot_order_entry_allowed(true)
            .build()
            .expect("permissions must build"),
        trading_constraints,
    );

    assert!(market.is_trading());
    assert!(market.allows_spot_order_entry());
    assert_eq!(market.tick_size(), Some(decimal("0.05")));
    assert_eq!(market.quote_scale(), Some(6));
    assert_eq!(market.min_notional_or(decimal("2")), decimal("10"));
    assert_eq!(
        market.min_quantity(OrderType::Limit),
        Some(decimal("0.001"))
    );
    assert_eq!(market.max_quantity(OrderType::Limit), Some(decimal("100")));
    assert_eq!(market.step_size(OrderType::Limit), Some(decimal("0.001")));
    assert_eq!(
        market.min_quantity(OrderType::Market),
        Some(decimal("0.01"))
    );
    assert_eq!(market.max_quantity(OrderType::Market), Some(decimal("50")));
    assert_eq!(
        market.step_size(OrderType::StopMarket),
        Some(decimal("0.01"))
    );

    let fallback_market = market_info(
        MarketStatus::PreLaunch,
        None,
        None,
        TradingPermissions::default(),
        TradingConstraints::builder()
            .lot_size(limit_lot_size)
            .build()
            .expect("fallback constraints must build"),
    );

    assert!(!fallback_market.is_trading());
    assert!(fallback_market.allows_spot_order_entry());
    assert_eq!(fallback_market.tick_size(), None);
    assert_eq!(fallback_market.quote_scale(), None);
    assert_eq!(fallback_market.min_notional_or(decimal("2")), decimal("2"));
    assert_eq!(
        fallback_market.min_quantity(OrderType::Market),
        Some(decimal("0.001"))
    );
    assert_eq!(
        fallback_market.max_quantity(OrderType::Market),
        Some(decimal("100"))
    );
    assert_eq!(
        fallback_market.step_size(OrderType::Market),
        Some(decimal("0.001"))
    );
}

#[test]
fn market_order_constraints_fall_back_per_field() {
    let market = market_info(
        MarketStatus::Trading,
        None,
        None,
        TradingPermissions::default(),
        TradingConstraints::builder()
            .lot_size(lot_size("0.001", "100", "0.001"))
            .market_lot_size(lot_size("0", "50", "0"))
            .build()
            .expect("constraints must build"),
    );

    assert_eq!(
        market.min_quantity(OrderType::Market),
        Some(decimal("0.001"))
    );
    assert_eq!(market.max_quantity(OrderType::Market), Some(decimal("50")));
    assert_eq!(market.step_size(OrderType::Market), Some(decimal("0.001")));
    assert_eq!(
        market.min_quantity(OrderType::StopMarket),
        Some(decimal("0.001"))
    );
    assert_eq!(
        market.max_quantity(OrderType::StopMarket),
        Some(decimal("50"))
    );
    assert_eq!(
        market.step_size(OrderType::StopMarket),
        Some(decimal("0.001"))
    );
}
