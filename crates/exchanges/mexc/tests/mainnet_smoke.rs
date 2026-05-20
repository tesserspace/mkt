use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mkt_core::{ApiCredentials, EventStream, ExchangeConfig, MarketDataEvent, Subscription};
use mkt_exchange_mexc::MexcClient;
use mkt_types::{
    Balance, ClientOrderId, KlineInterval, KlineRequest, LastPrice, MarketInfo, MarketQuantityMode,
    MarketStatus, Order, OrderKey, OrderQuantity, OrderSide, OrderStatus, OrderType, Symbol,
};
use rust_decimal::{Decimal, RoundingStrategy};
use tokio::time::timeout;

mod mainnet_supplemental_support;
use mainnet_supplemental_support::{
    cancel_spot_order, open_spot_orders, place_limit_sell, smoke_public_stream_event_set,
};

const MEXC_MAINNET_API_KEY: &str = "MEXC_MAINNET_API_KEY";
const MEXC_MAINNET_SECRET_KEY: &str = "MEXC_MAINNET_SECRET_KEY";
const MKT_MEXC_MAINNET_SMOKE: &str = "MKT_MEXC_MAINNET_SMOKE";
const MKT_SMOKE_TESTS_REQUIRED: &str = "MKT_SMOKE_TESTS_REQUIRED";
const SMOKE_SYMBOL: &str = "USDCUSDT";
const STREAM_COVERAGE_SYMBOL: &str = "BTCUSDT";
const BASE_ASSET: &str = "USDC";
const QUOTE_ASSET: &str = "USDT";
const REST_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const STREAM_EVENT_TIMEOUT: Duration = Duration::from_secs(25);
const STREAM_CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
#[ignore = "places real MEXC mainnet spot orders; run explicitly with MKT_MEXC_MAINNET_SMOKE=1"]
async fn mexc_mainnet_usdcusdt_smoke() {
    let Some(handle) = mainnet_handle() else {
        handle_missing_credentials();
        return;
    };
    if !mainnet_smoke_enabled() {
        eprintln!(
            "skipping MEXC mainnet smoke test: set {MKT_MEXC_MAINNET_SMOKE}=1 to allow live orders"
        );
        return;
    }

    let symbol = Symbol::spot(SMOKE_SYMBOL);
    let market = smoke_market(&handle, &symbol).await;
    let last_price = smoke_public_rest(&handle, &symbol, &market).await;
    smoke_public_stream(&handle, &symbol).await;

    let balances_before = balances(&handle).await;
    let quote_before = available_balance(&balances_before, QUOTE_ASSET);
    let base_before = available_balance(&balances_before, BASE_ASSET);
    let Some(buy_quantity) = planned_buy_quantity(quote_before, &market, last_price.price) else {
        eprintln!(
            "skipping MEXC live order flow: available {QUOTE_ASSET} {quote_before} cannot satisfy {SMOKE_SYMBOL} order constraints"
        );
        return;
    };

    let buy = place_market_buy(&handle, &symbol, buy_quantity).await;
    assert_terminal_or_open(&buy);
    let queried_buy = spot_order(&handle, symbol.clone(), order_query_key(&buy)).await;
    assert_eq!(queried_buy.symbol, symbol);
    let buy_fills = spot_fills(&handle, symbol.clone(), order_query_key(&buy)).await;
    assert!(
        !buy_fills.is_empty(),
        "MEXC market buy should produce at least one fill"
    );

    let balances_after_buy = balances(&handle).await;
    let base_after_buy = available_balance(&balances_after_buy, BASE_ASSET);
    let sell_quantity = planned_sell_quantity(base_before, base_after_buy, &market);
    assert!(
        sell_quantity > Decimal::ZERO,
        "MEXC market buy should increase available {BASE_ASSET}; before={base_before}, after={base_after_buy}"
    );

    let sell = place_market_sell(&handle, &symbol, sell_quantity).await;
    assert_terminal_or_open(&sell);
    let queried_sell = spot_order(&handle, symbol.clone(), order_query_key(&sell)).await;
    assert_eq!(queried_sell.symbol, symbol);
    let sell_fills = spot_fills(&handle, symbol.clone(), order_query_key(&sell)).await;
    assert!(
        !sell_fills.is_empty(),
        "MEXC market sell should produce at least one fill"
    );

    let open_orders = with_rest_timeout(
        "MEXC open spot orders request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .open_spot_orders(Some(&symbol)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC open spot orders request should succeed: {err}"));
    assert!(
        !open_orders
            .iter()
            .any(|order| { (order.id == buy.id || order.id == sell.id) && order.status.is_open() }),
        "smoke test market orders should not remain open"
    );
}

#[tokio::test]
#[ignore = "places real MEXC mainnet spot orders; run explicitly with MKT_MEXC_MAINNET_SMOKE=1"]
async fn mexc_mainnet_usdcusdt_supplemental_smoke() {
    let Some(handle) = mainnet_handle() else {
        handle_missing_credentials();
        return;
    };
    if !mainnet_smoke_enabled() {
        eprintln!(
            "skipping MEXC supplemental smoke test: set {MKT_MEXC_MAINNET_SMOKE}=1 to allow live orders"
        );
        return;
    }

    let symbol = Symbol::spot(SMOKE_SYMBOL);
    let market = smoke_market(&handle, &symbol).await;
    let last_price = with_rest_timeout(
        "MEXC last price request should finish within timeout",
        handle
            .market_data()
            .expect("MEXC handle should bind market data capability")
            .last_price(&symbol),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC last price request should succeed: {err}"));
    let all_prices = with_rest_timeout(
        "MEXC all last prices request should finish within timeout",
        handle
            .market_data()
            .expect("MEXC handle should bind market data capability")
            .last_prices(None),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC all last prices request should succeed: {err}"));
    assert!(all_prices.iter().any(|price| price.symbol == symbol));
    smoke_public_stream_event_set(&handle, &Symbol::spot(STREAM_COVERAGE_SYMBOL)).await;

    let before = balances(&handle).await;
    let quote_before = available_balance(&before, QUOTE_ASSET);
    let base_before = available_balance(&before, BASE_ASSET);
    let Some(buy_quantity) = planned_buy_quantity(quote_before, &market, last_price.price) else {
        eprintln!("skipping MEXC supplemental order flow: available {QUOTE_ASSET} {quote_before}");
        return;
    };
    let buy = place_market_buy(&handle, &symbol, buy_quantity).await;
    assert!(!spot_fills(&handle, symbol.clone(), order_query_key(&buy))
        .await
        .is_empty());

    let acquired = planned_sell_quantity(
        base_before,
        available_balance(&balances(&handle).await, BASE_ASSET),
        &market,
    );
    assert!(acquired > Decimal::ZERO, "should acquire {BASE_ASSET}");
    let limit_sell = place_limit_sell(
        &handle,
        &symbol,
        acquired,
        limit_sell_price(last_price.price, &market),
    )
    .await;
    let open_orders = open_spot_orders(&handle, &symbol).await;
    assert!(open_orders.iter().any(|order| order.id == limit_sell.id));
    let queried = spot_order(&handle, symbol.clone(), order_query_key(&limit_sell)).await;
    assert!(queried.status.is_open(), "limit sell should remain open");
    let _ = cancel_spot_order(&handle, &symbol, order_query_key(&limit_sell)).await;
    assert!(!open_spot_orders(&handle, &symbol)
        .await
        .iter()
        .any(|order| order.id == limit_sell.id));

    let cleanup_quantity = planned_sell_quantity(
        base_before,
        available_balance(&balances(&handle).await, BASE_ASSET),
        &market,
    );
    if cleanup_quantity > Decimal::ZERO {
        let cleanup = place_market_sell(&handle, &symbol, cleanup_quantity).await;
        assert_terminal_or_open(&cleanup);
    }
}

async fn smoke_public_rest(
    handle: &mkt_core::ExchangeHandle,
    symbol: &Symbol,
    market: &MarketInfo,
) -> LastPrice {
    let market_data = handle
        .market_data()
        .expect("MEXC handle should bind market data capability");

    let markets = with_rest_timeout(
        "MEXC markets request should finish within timeout",
        market_data.markets(),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC markets request should succeed: {err}"));
    assert!(
        markets.iter().any(|candidate| candidate.symbol == *symbol),
        "MEXC markets should include {SMOKE_SYMBOL}"
    );
    assert_eq!(market.base_asset, BASE_ASSET);
    assert_eq!(market.quote_asset, QUOTE_ASSET);

    let last_price = with_rest_timeout(
        "MEXC last price request should finish within timeout",
        market_data.last_price(symbol),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC last price request should succeed: {err}"));
    assert_positive_price(&last_price);

    let prices = with_rest_timeout(
        "MEXC scoped last prices request should finish within timeout",
        market_data.last_prices(Some(std::slice::from_ref(symbol))),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC scoped last prices request should succeed: {err}"));
    assert_eq!(prices.len(), 1);
    assert_positive_price(&prices[0]);

    let order_book = with_rest_timeout(
        "MEXC order book request should finish within timeout",
        market_data.order_book(symbol, Some(5)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC order book request should succeed: {err}"));
    assert!(
        !order_book.bids.is_empty(),
        "order book should include bids"
    );
    assert!(
        !order_book.asks.is_empty(),
        "order book should include asks"
    );

    let trades = with_rest_timeout(
        "MEXC recent trades request should finish within timeout",
        market_data.recent_trades(symbol, Some(5)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC recent trades request should succeed: {err}"));
    assert!(!trades.is_empty(), "recent trades should not be empty");

    let klines = with_rest_timeout(
        "MEXC klines request should finish within timeout",
        market_data.klines(
            KlineRequest::builder()
                .symbol(symbol.clone())
                .interval(KlineInterval::M1)
                .limit(Some(2))
                .build()
                .expect("smoke kline request should build"),
        ),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC klines request should succeed: {err}"));
    assert!(!klines.is_empty(), "klines should not be empty");

    last_price
}

async fn smoke_public_stream(handle: &mkt_core::ExchangeHandle, symbol: &Symbol) {
    let mut stream = timeout(
        STREAM_CONNECT_TIMEOUT,
        handle
            .public_stream()
            .expect("MEXC handle should bind public stream capability")
            .subscribe_public(vec![
                Subscription::LastPrice(symbol.clone()),
                Subscription::Trades(symbol.clone()),
                Subscription::AggTrades(symbol.clone()),
                Subscription::BookTicker(symbol.clone()),
                Subscription::MiniTicker(symbol.clone()),
                Subscription::OrderBook {
                    symbol: symbol.clone(),
                    depth: Some(5),
                },
                Subscription::OrderBookDeltas {
                    symbol: symbol.clone(),
                    max_update_interval: Some(Duration::from_millis(100)),
                },
            ]),
    )
    .await
    .unwrap_or_else(|_| panic!("MEXC public stream subscription should finish within timeout"))
    .unwrap_or_else(|err| panic!("MEXC public stream subscription should succeed: {err}"));

    timeout(
        STREAM_EVENT_TIMEOUT,
        next_stream_event(stream.as_mut(), symbol),
    )
    .await
    .unwrap_or_else(|_| panic!("MEXC public stream should emit a decoded event within timeout"))
    .unwrap_or_else(|message| panic!("{message}"));

    timeout(STREAM_CLOSE_TIMEOUT, stream.close())
        .await
        .unwrap_or_else(|_| panic!("MEXC public stream should close within timeout"))
        .unwrap_or_else(|err| panic!("MEXC public stream should close cleanly: {err}"));
}

async fn next_stream_event(stream: &mut dyn EventStream, symbol: &Symbol) -> Result<(), String> {
    loop {
        match stream.next().await {
            Ok(Some(event)) if event_symbol_matches(&event, symbol) => return Ok(()),
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err("MEXC public stream closed before a matching event".to_owned());
            }
            Err(err) => {
                return Err(format!("MEXC public stream event should decode: {err}"));
            }
        }
    }
}

fn event_symbol_matches(event: &MarketDataEvent, symbol: &Symbol) -> bool {
    match event {
        MarketDataEvent::LastPrice(event) => event.symbol == *symbol,
        MarketDataEvent::OrderBook(event) => event.symbol == *symbol,
        MarketDataEvent::OrderBookDelta(event) => event.symbol == *symbol,
        MarketDataEvent::Trade(event) => event.symbol == *symbol,
        MarketDataEvent::AggTrade(event) => event.symbol == *symbol,
        MarketDataEvent::BookTicker(event) => event.symbol == *symbol,
        MarketDataEvent::MiniTicker(event) => event.symbol == *symbol,
        MarketDataEvent::Kline(event) => event.symbol == *symbol,
        MarketDataEvent::BlockTrade(_)
        | MarketDataEvent::AveragePrice(_)
        | MarketDataEvent::Raw { .. } => false,
        _ => false,
    }
}

async fn smoke_market(handle: &mkt_core::ExchangeHandle, symbol: &Symbol) -> MarketInfo {
    let market = with_rest_timeout(
        "MEXC symbol market request should finish within timeout",
        handle
            .market_data()
            .expect("MEXC handle should bind market data capability")
            .market(symbol),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC symbol market request should succeed: {err}"))
    .unwrap_or_else(|| panic!("MEXC should list {SMOKE_SYMBOL}"));

    assert_eq!(market.status, MarketStatus::Trading);
    assert!(market.trading_permissions.allows_spot_order_entry());
    assert!(
        market
            .trading_permissions
            .supports_order_type(OrderType::Market),
        "{SMOKE_SYMBOL} should support market orders"
    );
    market
}

async fn balances(handle: &mkt_core::ExchangeHandle) -> Vec<Balance> {
    with_rest_timeout(
        "MEXC account balances request should finish within timeout",
        handle
            .account()
            .expect("MEXC handle should bind account capability")
            .balances(),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC account balances request should succeed: {err}"))
}

async fn place_market_buy(
    handle: &mkt_core::ExchangeHandle,
    symbol: &Symbol,
    quantity: OrderQuantity,
) -> Order {
    with_rest_timeout(
        "MEXC market buy request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .place_spot_order(
                mkt_types::SpotOrderRequest::builder()
                    .symbol(symbol.clone())
                    .side(OrderSide::Buy)
                    .order_type(OrderType::Market)
                    .quantity(quantity)
                    .client_order_id(Some(client_order_id("buy")))
                    .build()
                    .expect("smoke market buy request should build"),
            ),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC market buy request should succeed: {err}"))
}

async fn place_market_sell(
    handle: &mkt_core::ExchangeHandle,
    symbol: &Symbol,
    base_quantity: Decimal,
) -> Order {
    with_rest_timeout(
        "MEXC market sell request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .place_spot_order(
                mkt_types::SpotOrderRequest::builder()
                    .symbol(symbol.clone())
                    .side(OrderSide::Sell)
                    .order_type(OrderType::Market)
                    .quantity(OrderQuantity::Base(base_quantity))
                    .client_order_id(Some(client_order_id("sell")))
                    .build()
                    .expect("smoke market sell request should build"),
            ),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC market sell request should succeed: {err}"))
}

async fn spot_order(handle: &mkt_core::ExchangeHandle, symbol: Symbol, key: OrderKey) -> Order {
    with_rest_timeout(
        "MEXC spot order query should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .spot_order(mkt_types::SpotOrderQuery::new(symbol, key)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC spot order query should succeed: {err}"))
}

async fn spot_fills(
    handle: &mkt_core::ExchangeHandle,
    symbol: Symbol,
    key: OrderKey,
) -> Vec<mkt_types::Fill> {
    with_rest_timeout(
        "MEXC spot fills request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .spot_fills(mkt_types::SpotOrderQuery::new(symbol, key)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC spot fills request should succeed: {err}"))
}

async fn with_rest_timeout<F, T>(message: &'static str, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    timeout(REST_REQUEST_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| panic!("{message}"))
}

fn planned_buy_quantity(
    available_quote: Decimal,
    market: &MarketInfo,
    last_price: Decimal,
) -> Option<OrderQuantity> {
    let quote_to_spend = planned_quote_spend(available_quote, market)?;
    if !market.trading_permissions.supports_quantity_mode(
        MarketQuantityMode::Quote,
        OrderType::Market,
        OrderSide::Buy,
    ) {
        return planned_base_buy_quantity(quote_to_spend, last_price, market)
            .map(OrderQuantity::Base);
    }

    Some(OrderQuantity::Quote(truncate_to_quote_precision(
        quote_to_spend,
        market,
    )))
}

fn planned_quote_spend(available_quote: Decimal, market: &MarketInfo) -> Option<Decimal> {
    let affordable = (available_quote * balance_safety_factor()).min(max_quote_to_spend());
    let min_notional = market
        .trading_constraints
        .notional
        .as_ref()
        .and_then(|constraints| constraints.min_notional)
        .unwrap_or_else(default_min_notional);
    if affordable < min_notional {
        return None;
    }

    Some(affordable)
}

fn planned_base_buy_quantity(
    quote_to_spend: Decimal,
    last_price: Decimal,
    market: &MarketInfo,
) -> Option<Decimal> {
    if last_price <= Decimal::ZERO {
        return None;
    }

    let lot_size = market
        .trading_constraints
        .market_lot_size
        .as_ref()
        .or(market.trading_constraints.lot_size.as_ref());
    let mut quantity = quote_to_spend / last_price;
    if let Some(step) = lot_size.and_then(|lot_size| lot_size.step_size) {
        quantity = floor_to_step(quantity, step);
    }
    if let Some(min_quantity) = lot_size.and_then(|lot_size| lot_size.min_quantity) {
        quantity = quantity.max(min_quantity);
    }
    if quantity <= Decimal::ZERO || quantity * last_price > quote_to_spend {
        return None;
    }

    Some(quantity)
}

fn planned_sell_quantity(
    base_before: Decimal,
    base_after_buy: Decimal,
    market: &MarketInfo,
) -> Decimal {
    let acquired = base_after_buy - base_before;
    if acquired <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let quantity = market
        .trading_constraints
        .market_lot_size
        .as_ref()
        .or(market.trading_constraints.lot_size.as_ref())
        .and_then(|lot_size| lot_size.step_size)
        .map(|step| floor_to_step(acquired, step))
        .unwrap_or(acquired);

    let min_quantity = market
        .trading_constraints
        .market_lot_size
        .as_ref()
        .or(market.trading_constraints.lot_size.as_ref())
        .and_then(|lot_size| lot_size.min_quantity)
        .unwrap_or(Decimal::ZERO);
    if quantity < min_quantity {
        Decimal::ZERO
    } else {
        quantity
    }
}

fn floor_to_step(quantity: Decimal, step: Decimal) -> Decimal {
    if step <= Decimal::ZERO {
        return quantity;
    }
    (quantity / step).trunc() * step
}

fn max_quote_to_spend() -> Decimal {
    Decimal::new(110, 2)
}

fn balance_safety_factor() -> Decimal {
    Decimal::new(95, 2)
}

fn default_min_notional() -> Decimal {
    Decimal::new(1, 2)
}

fn truncate_to_quote_precision(quantity: Decimal, market: &MarketInfo) -> Decimal {
    let Some(precision) = market.quote_precision.or(market.quote_asset_precision) else {
        return quantity;
    };
    let Ok(scale) = u32::try_from(precision) else {
        return quantity;
    };
    quantity.round_dp_with_strategy(scale, RoundingStrategy::ToZero)
}

fn limit_sell_price(last_price: Decimal, market: &MarketInfo) -> Decimal {
    let price = last_price * Decimal::new(102, 2);
    truncate_to_quote_precision(price, market)
}

fn available_balance(balances: &[Balance], asset: &str) -> Decimal {
    balances
        .iter()
        .find(|balance| balance.asset == asset)
        .map(|balance| balance.available)
        .unwrap_or(Decimal::ZERO)
}

fn assert_positive_price(last_price: &LastPrice) {
    assert!(
        last_price.price > Decimal::ZERO,
        "last price should be positive"
    );
}

fn assert_terminal_or_open(order: &Order) {
    assert!(
        matches!(
            order.status,
            OrderStatus::New
                | OrderStatus::PartiallyFilled
                | OrderStatus::Filled
                | OrderStatus::Canceled
                | OrderStatus::Expired
        ),
        "unexpected MEXC order status: {:?}",
        order.status
    );
}

fn order_query_key(order: &Order) -> OrderKey {
    order
        .client_order_id
        .clone()
        .map(OrderKey::Client)
        .unwrap_or_else(|| OrderKey::Exchange(order.id.clone()))
}

fn mainnet_handle() -> Option<mkt_core::ExchangeHandle> {
    let credentials = mainnet_credentials()?;
    let config = ExchangeConfig::builder()
        .exchange_id(mkt_types::KnownExchange::Mexc)
        .credentials(credentials)
        .build()
        .expect("MEXC mainnet exchange config should build");

    Some(
        MexcClient::new(config)
            .expect("MEXC mainnet client should build")
            .into(),
    )
}

fn mainnet_credentials() -> Option<ApiCredentials> {
    let api_key = secret_value(MEXC_MAINNET_API_KEY)?;
    let secret = secret_value(MEXC_MAINNET_SECRET_KEY)?;

    Some(
        ApiCredentials::builder()
            .api_key(api_key)
            .secret(secret)
            .build()
            .expect("MEXC mainnet API credentials should build"),
    )
}

fn secret_value(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn client_order_id(prefix: &str) -> ClientOrderId {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH makes unique smoke client id impossible")
        .as_millis();
    ClientOrderId::new(format!("mktSmoke{prefix}{timestamp}"))
}

fn handle_missing_credentials() {
    if smoke_tests_required() {
        panic!(
            "MEXC mainnet smoke test credentials are required because {MKT_SMOKE_TESTS_REQUIRED} is set; configure {MEXC_MAINNET_API_KEY} and {MEXC_MAINNET_SECRET_KEY} in the process environment"
        );
    }

    eprintln!(
        "skipping MEXC mainnet smoke test: {MEXC_MAINNET_API_KEY} and {MEXC_MAINNET_SECRET_KEY} are not set"
    );
}

fn mainnet_smoke_enabled() -> bool {
    env_flag(MKT_MEXC_MAINNET_SMOKE)
}

fn smoke_tests_required() -> bool {
    env_flag(MKT_SMOKE_TESTS_REQUIRED)
}

fn env_flag(key: &str) -> bool {
    env::var(key).is_ok_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}
