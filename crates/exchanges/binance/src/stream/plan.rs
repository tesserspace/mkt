use std::{
    borrow::Borrow,
    collections::{btree_map::Entry, BTreeMap},
    time::Duration,
};

use mkt_core::{Result, Subscription};
use mkt_types::{KlineInterval, Symbol};

/// Binance combined-stream key.
///
/// The SDK surfaces raw websocket envelopes where `stream` is only a bare
/// stream name string, so the adapter keeps this typed key for manual routing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BinanceStreamName(String);

impl BinanceStreamName {
    fn trade(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "trade", operation)
    }

    fn agg_trade(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "aggTrade", operation)
    }

    fn block_trade(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "blockTrade", operation)
    }

    fn book_ticker(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "bookTicker", operation)
    }

    fn average_price(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "avgPrice", operation)
    }

    fn mini_ticker(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_stream(symbol, "miniTicker", operation)
    }

    fn partial_book_depth(
        symbol: &Symbol,
        depth: Option<u16>,
        operation: &'static str,
    ) -> Result<Self> {
        Ok(Self::new(format!(
            "{}@depth{}",
            Self::stream_symbol(symbol, operation)?,
            Self::partial_book_depth_levels(depth, operation)?
        )))
    }

    fn diff_book_depth(
        symbol: &Symbol,
        max_update_interval: Option<Duration>,
        operation: &'static str,
    ) -> Result<Self> {
        let symbol = Self::stream_symbol(symbol, operation)?;
        Ok(Self::new(match max_update_interval {
            Some(interval) if interval < Duration::from_millis(100) => {
                return Err(crate::error::invalid_field(
                    operation,
                    "max_update_interval",
                    "Binance spot diff depth streams do not support intervals below 100ms",
                ));
            }
            Some(interval) if interval < Duration::from_millis(1000) => {
                format!("{symbol}@depth@100ms")
            }
            Some(_) | None => format!("{symbol}@depth"),
        }))
    }

    fn kline(symbol: &Symbol, interval: KlineInterval, operation: &'static str) -> Result<Self> {
        Ok(Self::new(format!(
            "{}@kline_{}",
            Self::stream_symbol(symbol, operation)?,
            crate::convert::stream::stream_interval(interval, operation)?
        )))
    }

    fn symbol_stream(
        symbol: &Symbol,
        stream: &'static str,
        operation: &'static str,
    ) -> Result<Self> {
        Ok(Self::new(format!(
            "{}@{}",
            Self::stream_symbol(symbol, operation)?,
            stream
        )))
    }

    fn stream_symbol(symbol: &Symbol, operation: &'static str) -> Result<String> {
        let symbol = crate::convert::require_spot_symbol(symbol, operation)?;
        Ok(symbol.to_ascii_lowercase())
    }

    fn partial_book_depth_levels(depth: Option<u16>, operation: &'static str) -> Result<u16> {
        match depth.unwrap_or(20) {
            supported @ (5 | 10 | 20) => Ok(supported),
            other => Err(crate::error::invalid_field(
                operation,
                "depth",
                format!(
                    "Binance spot partial book streams support 5, 10, or 20 levels, got {other}"
                ),
            )),
        }
    }

    fn new(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for BinanceStreamName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for BinanceStreamName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<BinanceStreamName> for String {
    fn from(value: BinanceStreamName) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BinancePublicStreamRoute {
    Trade { projections: Vec<TradeProjection> },
    AggTrade,
    BlockTrade,
    BookTicker,
    OrderBook { symbol: Symbol },
    OrderBookDelta,
    AveragePrice,
    MiniTicker,
    Kline { interval: KlineInterval },
}

/// Internal projection from one Binance `@trade` payload into mkt events.
///
/// `LastPrice` and full trade subscriptions share the same upstream Binance
/// stream, so a route can project the payload into one or both event shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TradeProjection {
    LastPrice,
    Trade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BinancePublicStreamPlan {
    pub(super) stream_names: Vec<BinanceStreamName>,
    pub(super) routes: BTreeMap<BinanceStreamName, BinancePublicStreamRoute>,
}

impl BinancePublicStreamPlan {
    pub(super) fn build(subscriptions: &[Subscription], operation: &'static str) -> Result<Self> {
        BinancePublicStreamPlanBuilder::new(operation).build(subscriptions)
    }
}

struct BinancePublicStreamPlanBuilder {
    operation: &'static str,
    stream_names: Vec<BinanceStreamName>,
    routes: BTreeMap<BinanceStreamName, BinancePublicStreamRoute>,
}

impl BinancePublicStreamPlanBuilder {
    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            stream_names: Vec::new(),
            routes: BTreeMap::new(),
        }
    }

    fn build(mut self, subscriptions: &[Subscription]) -> Result<BinancePublicStreamPlan> {
        if subscriptions.is_empty() {
            return Err(crate::error::invalid_field(
                self.operation,
                "subscriptions",
                "at least one public subscription is required",
            ));
        }

        for subscription in subscriptions {
            let (stream_name, route) = self.route_for_subscription(subscription)?;
            self.insert_route(stream_name, route)?;
        }

        Ok(BinancePublicStreamPlan {
            stream_names: self.stream_names,
            routes: self.routes,
        })
    }

    fn route_for_subscription(
        &self,
        subscription: &Subscription,
    ) -> Result<(BinanceStreamName, BinancePublicStreamRoute)> {
        match subscription {
            Subscription::LastPrice(symbol) => Ok((
                BinanceStreamName::trade(symbol, self.operation)?,
                self.trade_route(TradeProjection::LastPrice),
            )),
            Subscription::OrderBook { symbol, depth } => Ok((
                BinanceStreamName::partial_book_depth(symbol, *depth, self.operation)?,
                BinancePublicStreamRoute::OrderBook {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::OrderBookDeltas {
                symbol,
                max_update_interval,
            } => Ok((
                BinanceStreamName::diff_book_depth(symbol, *max_update_interval, self.operation)?,
                BinancePublicStreamRoute::OrderBookDelta,
            )),
            Subscription::Trades(symbol) => Ok((
                BinanceStreamName::trade(symbol, self.operation)?,
                self.trade_route(TradeProjection::Trade),
            )),
            Subscription::AggTrades(symbol) => Ok((
                BinanceStreamName::agg_trade(symbol, self.operation)?,
                BinancePublicStreamRoute::AggTrade,
            )),
            Subscription::BlockTrades(symbol) => Ok((
                BinanceStreamName::block_trade(symbol, self.operation)?,
                BinancePublicStreamRoute::BlockTrade,
            )),
            Subscription::BookTicker(symbol) => Ok((
                BinanceStreamName::book_ticker(symbol, self.operation)?,
                BinancePublicStreamRoute::BookTicker,
            )),
            Subscription::AveragePrice(symbol) => Ok((
                BinanceStreamName::average_price(symbol, self.operation)?,
                BinancePublicStreamRoute::AveragePrice,
            )),
            Subscription::MiniTicker(symbol) => Ok((
                BinanceStreamName::mini_ticker(symbol, self.operation)?,
                BinancePublicStreamRoute::MiniTicker,
            )),
            Subscription::Klines(request) => Ok((
                BinanceStreamName::kline(&request.symbol, request.interval, self.operation)?,
                BinancePublicStreamRoute::Kline {
                    interval: request.interval,
                },
            )),
            _ => Err(crate::error::invalid_field(
                self.operation,
                "subscriptions",
                "unsupported Binance public stream subscription",
            )),
        }
    }

    fn insert_route(
        &mut self,
        stream_name: BinanceStreamName,
        route: BinancePublicStreamRoute,
    ) -> Result<()> {
        let operation = self.operation;
        match self.routes.entry(stream_name.clone()) {
            Entry::Vacant(entry) => {
                self.stream_names.push(stream_name);
                entry.insert(route);
                Ok(())
            }
            Entry::Occupied(mut entry) => Self::merge_route(operation, entry.get_mut(), route),
        }
    }

    fn merge_route(
        operation: &'static str,
        existing: &mut BinancePublicStreamRoute,
        route: BinancePublicStreamRoute,
    ) -> Result<()> {
        if existing == &route {
            return Ok(());
        }

        match (existing, route) {
            (
                BinancePublicStreamRoute::Trade { projections },
                BinancePublicStreamRoute::Trade {
                    projections: new_projections,
                },
            ) => {
                for projection in new_projections {
                    if !projections.contains(&projection) {
                        projections.push(projection);
                    }
                }
                Ok(())
            }
            _ => Err(crate::error::invalid_field(
                operation,
                "subscriptions",
                "conflicting Binance stream routes",
            )),
        }
    }

    fn trade_route(&self, projection: TradeProjection) -> BinancePublicStreamRoute {
        BinancePublicStreamRoute::Trade {
            projections: vec![projection],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mkt_core::Subscription;
    use mkt_types::{KlineInterval, KlineRequest, SettlementMode, Symbol};

    use super::*;

    const OPERATION: &str = "test.websocket";

    #[test]
    fn public_stream_plan_maps_unified_subscriptions_to_spot_streams() {
        let plan = BinancePublicStreamPlan::build(
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
                    max_update_interval: None,
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("AVAXUSDT"),
                    max_update_interval: Some(Duration::from_millis(100)),
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("LINKUSDT"),
                    max_update_interval: Some(Duration::from_millis(1000)),
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
            stream_names(&plan),
            vec![
                "btcusdt@trade",
                "ethusdt@depth10",
                "bnbusdt@trade",
                "adausdt@aggTrade",
                "dotusdt@blockTrade",
                "xrpusdt@bookTicker",
                "ltcusdt@depth",
                "avaxusdt@depth@100ms",
                "linkusdt@depth",
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
        let plan = BinancePublicStreamPlan::build(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::Trades(Symbol::spot("BTCUSDT")),
            ],
            OPERATION,
        )
        .expect("shared trade subscriptions should build");

        assert_eq!(stream_names(&plan), vec!["btcusdt@trade"]);
        assert!(matches!(
            plan.routes.get("btcusdt@trade"),
            Some(BinancePublicStreamRoute::Trade { projections })
                if projections == &vec![TradeProjection::LastPrice, TradeProjection::Trade]
        ));
    }

    #[test]
    fn public_stream_plan_rejects_non_spot_and_unsupported_depth() {
        let non_spot = BinancePublicStreamPlan::build(
            &[Subscription::Trades(Symbol::derivative(
                mkt_types::DerivativeKind::perpetual(SettlementMode::Linear),
                "BTCUSDT",
            ))],
            OPERATION,
        );
        assert!(non_spot.is_err());

        let bad_depth = BinancePublicStreamPlan::build(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: Some(50),
            }],
            OPERATION,
        );
        assert!(bad_depth.is_err());

        let unsupported_interval = BinancePublicStreamPlan::build(
            &[Subscription::OrderBookDeltas {
                symbol: Symbol::spot("BTCUSDT"),
                max_update_interval: Some(Duration::from_millis(50)),
            }],
            OPERATION,
        );
        assert!(unsupported_interval.is_err());
    }

    #[test]
    fn default_order_book_depth_uses_binance_max_partial_depth() {
        let plan = BinancePublicStreamPlan::build(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: None,
            }],
            OPERATION,
        )
        .expect("default order book depth should build");

        assert_eq!(stream_names(&plan), vec!["btcusdt@depth20"]);
    }

    fn stream_names(plan: &BinancePublicStreamPlan) -> Vec<&str> {
        plan.stream_names
            .iter()
            .map(BinanceStreamName::as_ref)
            .collect()
    }
}
