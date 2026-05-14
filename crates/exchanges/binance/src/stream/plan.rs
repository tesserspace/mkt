use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use mkt_core::{Result, Subscription};
use mkt_types::{BookDepthUpdateSpeed, KlineInterval, Symbol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BinancePublicStreamRoute {
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
pub(super) enum TradeOutput {
    LastPrice,
    Trade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BinancePublicStreamPlan {
    pub(super) stream_names: Vec<String>,
    pub(super) routes: BTreeMap<String, BinancePublicStreamRoute>,
}

pub(super) fn build_public_stream_plan(
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
                    crate::convert::stream::stream_interval(request.interval, operation)?
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

#[cfg(test)]
mod tests {
    use mkt_core::Subscription;
    use mkt_types::{BookDepthUpdateSpeed, KlineInterval, KlineRequest, SettlementMode, Symbol};

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
                mkt_types::DerivativeKind::perpetual(SettlementMode::Linear),
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
}
