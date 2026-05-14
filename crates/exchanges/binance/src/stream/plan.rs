use std::{
    borrow::Borrow,
    collections::{btree_map::Entry, BTreeMap},
};

use mkt_core::{Result, Subscription};
use mkt_types::{BookDepthUpdateSpeed, KlineInterval, Symbol};

/// Binance combined-stream key.
///
/// The SDK surfaces raw websocket envelopes where `stream` is only a bare
/// stream name string, so the adapter keeps this typed key for manual routing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BinanceStreamName(String);

impl BinanceStreamName {
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
    Trade { projection: TradeProjection },
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
    LastPriceAndTrade,
}

impl TradeProjection {
    pub(super) fn emits_last_price(self) -> bool {
        matches!(self, Self::LastPrice | Self::LastPriceAndTrade)
    }

    pub(super) fn emits_trade(self) -> bool {
        matches!(self, Self::Trade | Self::LastPriceAndTrade)
    }

    fn merge(self, other: Self) -> Self {
        if self == other {
            return self;
        }

        match (self, other) {
            (Self::LastPriceAndTrade, _) | (_, Self::LastPriceAndTrade) => Self::LastPriceAndTrade,
            (Self::LastPrice, Self::Trade) | (Self::Trade, Self::LastPrice) => {
                Self::LastPriceAndTrade
            }
            (Self::LastPrice, Self::LastPrice) => Self::LastPrice,
            (Self::Trade, Self::Trade) => Self::Trade,
        }
    }
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
                self.named(format!("{}@trade", self.stream_symbol(symbol)?)),
                self.trade_route(TradeProjection::LastPrice),
            )),
            Subscription::OrderBook { symbol, depth } => Ok((
                self.named(format!(
                    "{}@depth{}",
                    self.stream_symbol(symbol)?,
                    self.partial_book_depth(*depth)?
                )),
                BinancePublicStreamRoute::OrderBook {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::OrderBookDeltas { symbol, speed } => Ok((
                self.diff_depth_stream_name(symbol, *speed)?,
                BinancePublicStreamRoute::OrderBookDelta,
            )),
            Subscription::Trades(symbol) => Ok((
                self.named(format!("{}@trade", self.stream_symbol(symbol)?)),
                self.trade_route(TradeProjection::Trade),
            )),
            Subscription::AggTrades(symbol) => Ok((
                self.named(format!("{}@aggTrade", self.stream_symbol(symbol)?)),
                BinancePublicStreamRoute::AggTrade,
            )),
            Subscription::BlockTrades(symbol) => Ok((
                self.named(format!("{}@blockTrade", self.stream_symbol(symbol)?)),
                BinancePublicStreamRoute::BlockTrade,
            )),
            Subscription::BookTicker(symbol) => Ok((
                self.named(format!("{}@bookTicker", self.stream_symbol(symbol)?)),
                BinancePublicStreamRoute::BookTicker,
            )),
            Subscription::AveragePrice(symbol) => Ok((
                self.named(format!("{}@avgPrice", self.stream_symbol(symbol)?)),
                BinancePublicStreamRoute::AveragePrice,
            )),
            Subscription::MiniTicker(symbol) => Ok((
                self.named(format!("{}@miniTicker", self.stream_symbol(symbol)?)),
                BinancePublicStreamRoute::MiniTicker,
            )),
            Subscription::Klines(request) => Ok((
                self.named(format!(
                    "{}@kline_{}",
                    self.stream_symbol(&request.symbol)?,
                    crate::convert::stream::stream_interval(request.interval, self.operation)?
                )),
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
                BinancePublicStreamRoute::Trade { projection },
                BinancePublicStreamRoute::Trade {
                    projection: new_projection,
                },
            ) => {
                *projection = (*projection).merge(new_projection);
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
        BinancePublicStreamRoute::Trade { projection }
    }

    fn stream_symbol(&self, symbol: &Symbol) -> Result<String> {
        let symbol = crate::convert::require_spot_symbol(symbol, self.operation)?;
        Ok(symbol.to_ascii_lowercase())
    }

    fn diff_depth_stream_name(
        &self,
        symbol: &Symbol,
        speed: Option<BookDepthUpdateSpeed>,
    ) -> Result<BinanceStreamName> {
        let symbol = self.stream_symbol(symbol)?;
        Ok(self.named(match speed {
            Some(speed) => {
                let speed_name: &'static str = speed.into();
                format!("{symbol}@depth@{speed_name}")
            }
            None => format!("{symbol}@depth"),
        }))
    }

    fn partial_book_depth(&self, depth: Option<u32>) -> Result<u32> {
        match depth.unwrap_or(20) {
            supported @ (5 | 10 | 20) => Ok(supported),
            other => Err(crate::error::invalid_field(
                self.operation,
                "depth",
                format!(
                    "Binance spot partial book streams support 5, 10, or 20 levels, got {other}"
                ),
            )),
        }
    }

    fn named(&self, value: String) -> BinanceStreamName {
        BinanceStreamName::new(value)
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
            Some(BinancePublicStreamRoute::Trade {
                projection: TradeProjection::LastPriceAndTrade,
            })
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
