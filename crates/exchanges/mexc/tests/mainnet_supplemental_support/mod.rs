use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mkt_core::{EventStream, MarketDataEvent, Subscription};
use mkt_types::{
    ClientOrderId, Order, OrderKey, OrderQuantity, OrderSide, OrderType, SpotCancelOrderRequest,
    Symbol, TimeInForce,
};
use rust_decimal::Decimal;
use tokio::time::timeout;

const REST_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const STREAM_COVERAGE_TIMEOUT: Duration = Duration::from_secs(60);
const STREAM_CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn smoke_public_stream_event_set(handle: &mkt_core::ExchangeHandle, symbol: &Symbol) {
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
        STREAM_COVERAGE_TIMEOUT,
        collect_stream_event_mask(stream.as_mut(), symbol),
    )
    .await
    .unwrap_or_else(|_| panic!("MEXC public stream should emit all covered event types"))
    .unwrap_or_else(|message| panic!("{message}"));

    timeout(STREAM_CLOSE_TIMEOUT, stream.close())
        .await
        .unwrap_or_else(|_| panic!("MEXC public stream should close within timeout"))
        .unwrap_or_else(|err| panic!("MEXC public stream should close cleanly: {err}"));
}

pub async fn place_limit_sell(
    handle: &mkt_core::ExchangeHandle,
    symbol: &Symbol,
    quantity: Decimal,
    price: Decimal,
) -> Order {
    with_rest_timeout(
        "MEXC limit sell request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .place_spot_order(
                mkt_types::SpotOrderRequest::builder()
                    .symbol(symbol.clone())
                    .side(OrderSide::Sell)
                    .order_type(OrderType::Limit)
                    .quantity(OrderQuantity::Base(quantity))
                    .price(Some(price))
                    .time_in_force(Some(TimeInForce::Gtc))
                    .client_order_id(Some(client_order_id("limitSell")))
                    .build()
                    .expect("smoke limit sell request should build"),
            ),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC limit sell request should succeed: {err}"))
}

pub async fn cancel_spot_order(
    handle: &mkt_core::ExchangeHandle,
    symbol: &Symbol,
    key: OrderKey,
) -> Order {
    with_rest_timeout(
        "MEXC cancel spot order request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .cancel_spot_order(SpotCancelOrderRequest::new(symbol.clone(), key)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC cancel spot order request should succeed: {err}"))
}

pub async fn open_spot_orders(handle: &mkt_core::ExchangeHandle, symbol: &Symbol) -> Vec<Order> {
    with_rest_timeout(
        "MEXC open spot orders request should finish within timeout",
        handle
            .spot_trading()
            .expect("MEXC handle should bind spot trading capability")
            .open_spot_orders(Some(symbol)),
    )
    .await
    .unwrap_or_else(|err| panic!("MEXC open spot orders request should succeed: {err}"))
}

async fn with_rest_timeout<F, T>(message: &'static str, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    timeout(REST_REQUEST_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| panic!("{message}"))
}

const LAST_PRICE_EVENT: u8 = 1;
const TRADE_EVENT: u8 = 1 << 1;
const AGG_TRADE_EVENT: u8 = 1 << 2;
const BOOK_TICKER_EVENT: u8 = 1 << 3;
const MINI_TICKER_EVENT: u8 = 1 << 4;
const ORDER_BOOK_EVENT: u8 = 1 << 5;
const ORDER_BOOK_DELTA_EVENT: u8 = 1 << 6;
const REQUIRED_STREAM_EVENT_MASK: u8 = TRADE_EVENT | BOOK_TICKER_EVENT | ORDER_BOOK_EVENT;
const MIN_STREAM_EVENT_TYPES: u32 = 4;

async fn collect_stream_event_mask(
    stream: &mut dyn EventStream,
    symbol: &Symbol,
) -> Result<u8, String> {
    let mut mask = 0;
    while !has_required_stream_coverage(mask) {
        match stream.next().await {
            Ok(Some(event)) if event_symbol_matches(&event, symbol) => {
                mask |= stream_event_mask(&event);
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err("MEXC public stream closed before coverage completed".to_owned())
            }
            Err(err) => return Err(format!("MEXC public stream event should decode: {err}")),
        }
    }
    Ok(mask)
}

fn has_required_stream_coverage(mask: u8) -> bool {
    mask & REQUIRED_STREAM_EVENT_MASK == REQUIRED_STREAM_EVENT_MASK
        && mask.count_ones() >= MIN_STREAM_EVENT_TYPES
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
        _ => false,
    }
}

fn stream_event_mask(event: &MarketDataEvent) -> u8 {
    match event {
        MarketDataEvent::LastPrice(_) => LAST_PRICE_EVENT,
        MarketDataEvent::Trade(_) => TRADE_EVENT,
        MarketDataEvent::AggTrade(_) => AGG_TRADE_EVENT,
        MarketDataEvent::BookTicker(_) => BOOK_TICKER_EVENT,
        MarketDataEvent::MiniTicker(_) => MINI_TICKER_EVENT,
        MarketDataEvent::OrderBook(_) => ORDER_BOOK_EVENT,
        MarketDataEvent::OrderBookDelta(_) => ORDER_BOOK_DELTA_EVENT,
        _ => 0,
    }
}

fn client_order_id(prefix: &str) -> ClientOrderId {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH makes unique smoke client id impossible")
        .as_millis();
    ClientOrderId::new(format!("mktSmoke{prefix}{timestamp}"))
}
