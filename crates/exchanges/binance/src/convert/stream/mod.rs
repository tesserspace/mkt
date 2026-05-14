use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use mkt_core::{MarketDataEvent, RawPayload, Result, Subscription};
use mkt_types::{BookDepthUpdateSpeed, ExchangeId, KlineInterval, KnownExchange, Symbol};
use serde_json::Value;

mod book;
mod kline;
mod ticker;
mod trade;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BinancePublicStreamRoute {
    Trade { outputs: BTreeSet<TradeOutput> },
    AggTrade,
    BlockTrade,
    BookTicker,
    OrderBook { symbol: Symbol },
    OrderBookDelta,
    AveragePrice,
    MiniTicker,
    Kline { interval: KlineInterval },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TradeOutput {
    LastPrice,
    Trade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BinancePublicStreamPlan {
    pub(crate) stream_names: Vec<String>,
    pub(crate) routes: BTreeMap<String, BinancePublicStreamRoute>,
}

pub(crate) fn build_public_stream_plan(
    subscriptions: &[Subscription],
    operation: &'static str,
) -> Result<BinancePublicStreamPlan> {
    if subscriptions.is_empty() {
        return Err(crate::error::invalid_field(
            operation,
            "subscriptions",
            "at least one public subscription is required",
        ));
    }

    let mut stream_names = Vec::new();
    let mut routes = BTreeMap::new();

    for subscription in subscriptions {
        let (stream_name, route) = match subscription {
            Subscription::LastPrice(symbol) => (
                format!("{}@trade", stream_symbol(symbol, operation)?),
                trade_route(TradeOutput::LastPrice),
            ),
            Subscription::OrderBook { symbol, depth } => (
                format!(
                    "{}@depth{}",
                    stream_symbol(symbol, operation)?,
                    partial_book_depth(*depth, operation)?
                ),
                BinancePublicStreamRoute::OrderBook {
                    symbol: symbol.clone(),
                },
            ),
            Subscription::OrderBookDeltas { symbol, speed } => (
                diff_depth_stream_name(symbol, *speed, operation)?,
                BinancePublicStreamRoute::OrderBookDelta,
            ),
            Subscription::Trades(symbol) => (
                format!("{}@trade", stream_symbol(symbol, operation)?),
                trade_route(TradeOutput::Trade),
            ),
            Subscription::AggTrades(symbol) => (
                format!("{}@aggTrade", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::AggTrade,
            ),
            Subscription::BlockTrades(symbol) => (
                format!("{}@blockTrade", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::BlockTrade,
            ),
            Subscription::BookTicker(symbol) => (
                format!("{}@bookTicker", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::BookTicker,
            ),
            Subscription::AveragePrice(symbol) => (
                format!("{}@avgPrice", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::AveragePrice,
            ),
            Subscription::MiniTicker(symbol) => (
                format!("{}@miniTicker", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::MiniTicker,
            ),
            Subscription::Klines(request) => (
                format!(
                    "{}@kline_{}",
                    stream_symbol(&request.symbol, operation)?,
                    kline::stream_interval(request.interval, operation)?
                ),
                BinancePublicStreamRoute::Kline {
                    interval: request.interval,
                },
            ),
            _ => {
                return Err(crate::error::invalid_field(
                    operation,
                    "subscriptions",
                    "unsupported Binance public stream subscription",
                ));
            }
        };

        insert_route(
            stream_name,
            route,
            &mut stream_names,
            &mut routes,
            operation,
        )?;
    }

    Ok(BinancePublicStreamPlan {
        stream_names,
        routes,
    })
}

pub(crate) fn market_data_events_from_ws_text(
    raw: &str,
    routes: &BTreeMap<String, BinancePublicStreamRoute>,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    let value: Value = serde_json::from_str(raw).map_err(|err| {
        crate::error::decode_error(operation, format!("invalid websocket JSON: {err}"))
    })?;
    let Some(stream_name) = value.get("stream").and_then(Value::as_str) else {
        return Ok(vec![raw_event(raw)]);
    };
    let Some(payload) = value.get("data") else {
        return Ok(vec![raw_event(raw)]);
    };
    let Some(route) = routes.get(stream_name) else {
        return Ok(vec![raw_event(raw)]);
    };

    match route {
        BinancePublicStreamRoute::Trade { outputs } => {
            trade::trade_events_from_value(payload, outputs, operation)
        }
        BinancePublicStreamRoute::AggTrade => Ok(vec![MarketDataEvent::AggTrade(
            trade::agg_trade_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::BlockTrade => Ok(vec![MarketDataEvent::BlockTrade(
            trade::block_trade_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::BookTicker => Ok(vec![MarketDataEvent::BookTicker(
            book::book_ticker_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::OrderBook { symbol } => Ok(vec![MarketDataEvent::OrderBook(
            book::order_book_from_value(symbol, payload, operation)?,
        )]),
        BinancePublicStreamRoute::OrderBookDelta => Ok(vec![MarketDataEvent::OrderBookDelta(
            book::order_book_delta_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::AveragePrice => Ok(vec![MarketDataEvent::AveragePrice(
            ticker::average_price_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::MiniTicker => Ok(vec![MarketDataEvent::MiniTicker(
            ticker::mini_ticker_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::Kline { interval } => Ok(vec![MarketDataEvent::Kline(
            kline::kline_from_value(payload, *interval, operation)?,
        )]),
    }
}

fn insert_route(
    stream_name: String,
    route: BinancePublicStreamRoute,
    stream_names: &mut Vec<String>,
    routes: &mut BTreeMap<String, BinancePublicStreamRoute>,
    operation: &'static str,
) -> Result<()> {
    match routes.entry(stream_name.clone()) {
        Entry::Vacant(entry) => {
            stream_names.push(stream_name);
            entry.insert(route);
            Ok(())
        }
        Entry::Occupied(mut entry) => merge_route(entry.get_mut(), route, operation),
    }
}

fn merge_route(
    existing: &mut BinancePublicStreamRoute,
    route: BinancePublicStreamRoute,
    operation: &'static str,
) -> Result<()> {
    if existing == &route {
        return Ok(());
    }

    match (existing, route) {
        (
            BinancePublicStreamRoute::Trade { outputs },
            BinancePublicStreamRoute::Trade {
                outputs: new_outputs,
            },
        ) => {
            outputs.extend(new_outputs);
            Ok(())
        }
        _ => Err(crate::error::invalid_field(
            operation,
            "subscriptions",
            "conflicting Binance stream routes",
        )),
    }
}

fn trade_route(output: TradeOutput) -> BinancePublicStreamRoute {
    let mut outputs = BTreeSet::new();
    outputs.insert(output);
    BinancePublicStreamRoute::Trade { outputs }
}

fn stream_symbol(symbol: &Symbol, operation: &'static str) -> Result<String> {
    let symbol = crate::convert::require_spot_symbol(symbol, operation)?;
    Ok(symbol.to_ascii_lowercase())
}

fn diff_depth_stream_name(
    symbol: &Symbol,
    speed: Option<BookDepthUpdateSpeed>,
    operation: &'static str,
) -> Result<String> {
    let symbol = stream_symbol(symbol, operation)?;
    Ok(match speed {
        Some(speed) => {
            let speed_name: &'static str = speed.into();
            format!("{symbol}@depth@{speed_name}")
        }
        None => format!("{symbol}@depth"),
    })
}

fn partial_book_depth(depth: Option<u32>, operation: &'static str) -> Result<u32> {
    match depth.unwrap_or(20) {
        supported @ (5 | 10 | 20) => Ok(supported),
        other => Err(crate::error::invalid_field(
            operation,
            "depth",
            format!("Binance spot partial book streams support 5, 10, or 20 levels, got {other}"),
        )),
    }
}

fn raw_event(raw: &str) -> MarketDataEvent {
    MarketDataEvent::Raw {
        exchange_id: ExchangeId::from(KnownExchange::Binance),
        payload: RawPayload::Text(raw.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use mkt_core::{MarketDataEvent, RawPayload, Subscription};
    use mkt_types::{
        AggTrade, AveragePrice, BlockTrade, BookDepthUpdateSpeed, BookTicker, Kline, KlineInterval,
        KlineRequest, LastPrice, MiniTicker, OrderBook, OrderBookDelta, Symbol, Trade, TradeSide,
    };
    use rust_decimal::Decimal;

    use super::*;

    const OPERATION: &str = "test.websocket";

    #[test]
    fn public_stream_plan_maps_unified_subscriptions_to_spot_streams() {
        let plan = build_public_stream_plan(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::OrderBook {
                    symbol: Symbol::spot("ETHUSDT"),
                    depth: Some(10),
                },
                Subscription::Trades(Symbol::spot("BNBUSDT")),
                Subscription::AggTrades(Symbol::spot("ADAUSDT")),
                Subscription::BlockTrades(Symbol::spot("DOTUSDT")),
                Subscription::BookTicker(Symbol::spot("XRPUSDT")),
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("LTCUSDT"),
                    speed: None,
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("AVAXUSDT"),
                    speed: Some(BookDepthUpdateSpeed::Ms100),
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("LINKUSDT"),
                    speed: Some(BookDepthUpdateSpeed::Ms1000),
                },
                Subscription::AveragePrice(Symbol::spot("DOGEUSDT")),
                Subscription::MiniTicker(Symbol::spot("MATICUSDT")),
                Subscription::Klines(
                    KlineRequest::builder()
                        .symbol(Symbol::spot("SOLUSDT"))
                        .interval(KlineInterval::M1)
                        .build()
                        .expect("test kline request should build"),
                ),
            ],
            OPERATION,
        )
        .expect("supported subscriptions should build");

        assert_eq!(
            plan.stream_names,
            vec![
                "btcusdt@trade",
                "ethusdt@depth10",
                "bnbusdt@trade",
                "adausdt@aggTrade",
                "dotusdt@blockTrade",
                "xrpusdt@bookTicker",
                "ltcusdt@depth",
                "avaxusdt@depth@100ms",
                "linkusdt@depth@1000ms",
                "dogeusdt@avgPrice",
                "maticusdt@miniTicker",
                "solusdt@kline_1m",
            ]
        );
        assert!(matches!(
            plan.routes.get("solusdt@kline_1m"),
            Some(BinancePublicStreamRoute::Kline {
                interval: KlineInterval::M1
            })
        ));
    }

    #[test]
    fn last_price_and_trades_share_one_trade_stream() {
        let plan = build_public_stream_plan(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::Trades(Symbol::spot("BTCUSDT")),
            ],
            OPERATION,
        )
        .expect("shared trade subscriptions should build");

        assert_eq!(plan.stream_names, vec!["btcusdt@trade"]);
        assert!(matches!(
            plan.routes.get("btcusdt@trade"),
            Some(BinancePublicStreamRoute::Trade { outputs })
                if outputs.contains(&TradeOutput::LastPrice)
                    && outputs.contains(&TradeOutput::Trade)
        ));
    }

    #[test]
    fn public_stream_plan_rejects_non_spot_and_unsupported_depth() {
        let non_spot = build_public_stream_plan(
            &[Subscription::Trades(Symbol::derivative(
                mkt_types::DerivativeKind::perpetual(mkt_types::SettlementMode::Linear),
                "BTCUSDT",
            ))],
            OPERATION,
        );
        assert!(non_spot.is_err());

        let bad_depth = build_public_stream_plan(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: Some(50),
            }],
            OPERATION,
        );
        assert!(bad_depth.is_err());
    }

    #[test]
    fn trade_payload_emits_all_requested_trade_outputs() {
        let plan = build_public_stream_plan(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::Trades(Symbol::spot("BTCUSDT")),
            ],
            OPERATION,
        )
        .expect("shared trade subscriptions should build");

        let events = market_data_events_from_ws_text(
            r#"{"stream":"btcusdt@trade","data":{"e":"trade","E":1672515782136,"s":"BTCUSDT","t":12345,"p":"0.001","q":"100","T":1672515782136,"m":true,"M":true}}"#,
            &plan.routes,
            OPERATION,
        )
        .expect("trade should map");

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            MarketDataEvent::LastPrice(LastPrice { symbol, price, .. })
                if *symbol == Symbol::spot("BTCUSDT")
                    && *price == Decimal::from_str("0.001").expect("valid decimal")
        ));
        assert!(matches!(
            &events[1],
            MarketDataEvent::Trade(Trade { symbol, id, side: TradeSide::Sell, .. })
                if *symbol == Symbol::spot("BTCUSDT") && id.as_deref() == Some("12345")
        ));
    }

    #[test]
    fn websocket_payloads_map_to_domain_events() {
        let plan = build_public_stream_plan(
            &[
                Subscription::MiniTicker(Symbol::spot("BTCUSDT")),
                Subscription::OrderBook {
                    symbol: Symbol::spot("ETHUSDT"),
                    depth: Some(5),
                },
                Subscription::AggTrades(Symbol::spot("BNBUSDT")),
                Subscription::BlockTrades(Symbol::spot("DOTUSDT")),
                Subscription::BookTicker(Symbol::spot("XRPUSDT")),
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("ADAUSDT"),
                    speed: None,
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("AVAXUSDT"),
                    speed: Some(BookDepthUpdateSpeed::Ms100),
                },
                Subscription::AveragePrice(Symbol::spot("LTCUSDT")),
                Subscription::Klines(
                    KlineRequest::builder()
                        .symbol(Symbol::spot("SOLUSDT"))
                        .interval(KlineInterval::M1)
                        .build()
                        .expect("test kline request should build"),
                ),
            ],
            OPERATION,
        )
        .expect("supported subscriptions should build");

        let mini_ticker = one_event(
            r#"{"stream":"btcusdt@miniTicker","data":{"e":"24hrMiniTicker","E":1672515782136,"s":"BTCUSDT","c":"431.50000000","o":"400.00000000","h":"450.00000000","l":"390.00000000","v":"1200.5","q":"510000.25"}}"#,
            &plan.routes,
        );
        assert!(matches!(
            mini_ticker,
            MarketDataEvent::MiniTicker(MiniTicker { symbol, close, .. })
                if symbol == Symbol::spot("BTCUSDT")
                    && close == Decimal::from_str("431.50000000").expect("valid decimal")
        ));

        let order_book = one_event(
            r#"{"stream":"ethusdt@depth5","data":{"lastUpdateId":160,"bids":[["0.0024","10"]],"asks":[["0.0026","100"]]}}"#,
            &plan.routes,
        );
        assert!(matches!(
            order_book,
            MarketDataEvent::OrderBook(OrderBook { symbol, bids, asks, last_update_id, .. })
                if symbol == Symbol::spot("ETHUSDT")
                    && bids.len() == 1
                    && asks.len() == 1
                    && last_update_id.as_deref() == Some("160")
        ));

        let agg_trade = one_event(
            r#"{"stream":"bnbusdt@aggTrade","data":{"e":"aggTrade","E":1672515782136,"s":"BNBUSDT","a":123,"p":"0.001","q":"100","f":1000,"l":1005,"T":1672515782000,"m":false,"M":true}}"#,
            &plan.routes,
        );
        assert!(matches!(
            agg_trade,
            MarketDataEvent::AggTrade(AggTrade { symbol, id, first_trade_id, last_trade_id, side: TradeSide::Buy, .. })
                if symbol == Symbol::spot("BNBUSDT")
                    && id.as_deref() == Some("123")
                    && first_trade_id.as_deref() == Some("1000")
                    && last_trade_id.as_deref() == Some("1005")
        ));

        let block_trade = one_event(
            r#"{"stream":"dotusdt@blockTrade","data":{"e":"blockTrade","E":1672515782136,"s":"DOTUSDT","t":555,"p":"7.25","q":"10","T":1672515782000,"m":true}}"#,
            &plan.routes,
        );
        assert!(matches!(
            block_trade,
            MarketDataEvent::BlockTrade(BlockTrade { symbol, id, side: TradeSide::Sell, .. })
                if symbol == Symbol::spot("DOTUSDT") && id.as_deref() == Some("555")
        ));

        let book_ticker = one_event(
            r#"{"stream":"xrpusdt@bookTicker","data":{"u":400900217,"s":"XRPUSDT","b":"0.25","B":"200","a":"0.26","A":"300"}}"#,
            &plan.routes,
        );
        assert!(matches!(
            book_ticker,
            MarketDataEvent::BookTicker(BookTicker { symbol, bid_price, ask_price, last_update_id, .. })
                if symbol == Symbol::spot("XRPUSDT")
                    && bid_price == Decimal::from_str("0.25").expect("valid decimal")
                    && ask_price == Decimal::from_str("0.26").expect("valid decimal")
                    && last_update_id.as_deref() == Some("400900217")
        ));

        let order_book_delta = one_event(
            r#"{"stream":"adausdt@depth","data":{"e":"depthUpdate","E":1672515782136,"s":"ADAUSDT","U":157,"u":160,"b":[["0.0024","10"]],"a":[["0.0026","0"]]}}"#,
            &plan.routes,
        );
        assert!(matches!(
            order_book_delta,
            MarketDataEvent::OrderBookDelta(OrderBookDelta { symbol, first_update_id, last_update_id, bids, asks, .. })
                if symbol == Symbol::spot("ADAUSDT")
                    && first_update_id.as_deref() == Some("157")
                    && last_update_id.as_deref() == Some("160")
                    && bids.len() == 1
                    && asks.len() == 1
        ));

        let fast_order_book_delta = one_event(
            r#"{"stream":"avaxusdt@depth@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"AVAXUSDT","U":257,"u":260,"b":[["11.1","5"]],"a":[["11.2","7"]]}}"#,
            &plan.routes,
        );
        assert!(matches!(
            fast_order_book_delta,
            MarketDataEvent::OrderBookDelta(OrderBookDelta { symbol, last_update_id, .. })
                if symbol == Symbol::spot("AVAXUSDT") && last_update_id.as_deref() == Some("260")
        ));

        let average_price = one_event(
            r#"{"stream":"ltcusdt@avgPrice","data":{"e":"avgPrice","E":1672515782136,"s":"LTCUSDT","i":"5m","w":"81.23","T":1672515782000}}"#,
            &plan.routes,
        );
        assert!(matches!(
            average_price,
            MarketDataEvent::AveragePrice(AveragePrice { symbol, interval, price, .. })
                if symbol == Symbol::spot("LTCUSDT")
                    && interval.as_deref() == Some("5m")
                    && price == Decimal::from_str("81.23").expect("valid decimal")
        ));

        let kline = one_event(
            r#"{"stream":"solusdt@kline_1m","data":{"e":"kline","E":1672515782136,"s":"SOLUSDT","k":{"t":1672515780000,"T":1672515839999,"s":"SOLUSDT","i":"1m","f":100,"L":200,"o":"0.0010","c":"0.0020","h":"0.0025","l":"0.0015","v":"1000","n":100,"x":false,"q":"1.0000","V":"500","Q":"0.500","B":"0"}}}"#,
            &plan.routes,
        );
        assert!(matches!(
            kline,
            MarketDataEvent::Kline(Kline { symbol, interval: KlineInterval::M1, closed: false, .. })
                if symbol == Symbol::spot("SOLUSDT")
        ));
    }

    #[test]
    fn non_stream_messages_are_preserved_as_raw_events() {
        let events = market_data_events_from_ws_text(
            r#"{"result":null,"id":"subscribe-1"}"#,
            &BTreeMap::new(),
            OPERATION,
        )
        .expect("control messages should not fail decoding");

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            MarketDataEvent::Raw {
                payload: RawPayload::Text(raw),
                ..
            } if raw.contains("subscribe-1")
        ));
    }

    #[test]
    fn default_order_book_depth_uses_binance_max_partial_depth() {
        let plan = build_public_stream_plan(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: None,
            }],
            OPERATION,
        )
        .expect("default order book depth should build");

        assert_eq!(plan.stream_names, vec!["btcusdt@depth20"]);
    }

    fn one_event(
        raw: &str,
        routes: &BTreeMap<String, BinancePublicStreamRoute>,
    ) -> MarketDataEvent {
        let mut events = market_data_events_from_ws_text(raw, routes, OPERATION)
            .expect("websocket payload should map");
        assert_eq!(events.len(), 1);
        events.remove(0)
    }
}
