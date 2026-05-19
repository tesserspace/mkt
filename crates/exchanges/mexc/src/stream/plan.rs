use std::{
    borrow::Borrow,
    collections::{btree_map::Entry, BTreeMap},
    time::Duration,
};

use mkt_core::{Result, Subscription};
use mkt_types::{KlineInterval, MarketKind, Symbol};

const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct MexcChannel(String);

impl MexcChannel {
    fn aggre_deals(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::timed_symbol_channel("spot@public.aggre.deals.v3.api.pb", symbol, operation)
    }

    fn book_ticker(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::timed_symbol_channel("spot@public.aggre.bookTicker.v3.api.pb", symbol, operation)
    }

    fn mini_ticker(symbol: &Symbol, operation: &'static str) -> Result<Self> {
        Self::symbol_channel("spot@public.miniTicker.v3.api.pb", symbol, operation)
    }

    fn kline(symbol: &Symbol, interval: KlineInterval, operation: &'static str) -> Result<Self> {
        Ok(Self::new(format!(
            "spot@public.kline.v3.api.pb@{}@{}",
            Self::stream_symbol(symbol, operation)?,
            crate::convert::stream::stream_interval(interval, operation)?
        )))
    }

    fn order_book(symbol: &Symbol, depth: Option<u16>, operation: &'static str) -> Result<Self> {
        Ok(Self::new(format!(
            "spot@public.limit.depth.v3.api.pb@{}@{}",
            Self::stream_symbol(symbol, operation)?,
            Self::limit_depth(depth, operation)?
        )))
    }

    fn order_book_delta(
        symbol: &Symbol,
        max_update_interval: Option<Duration>,
        operation: &'static str,
    ) -> Result<Self> {
        Self::validate_delta_interval(max_update_interval, operation)?;
        Ok(Self::new(format!(
            "spot@public.aggre.depth.v3.api.pb@{}@{}",
            Self::aggre_depth_interval(max_update_interval),
            Self::stream_symbol(symbol, operation)?
        )))
    }

    fn timed_symbol_channel(
        prefix: &'static str,
        symbol: &Symbol,
        operation: &'static str,
    ) -> Result<Self> {
        Ok(Self::new(format!(
            "{prefix}@100ms@{}",
            Self::stream_symbol(symbol, operation)?
        )))
    }

    fn symbol_channel(
        prefix: &'static str,
        symbol: &Symbol,
        operation: &'static str,
    ) -> Result<Self> {
        Ok(Self::new(format!(
            "{prefix}@{}",
            Self::stream_symbol(symbol, operation)?
        )))
    }

    fn stream_symbol(symbol: &Symbol, operation: &'static str) -> Result<String> {
        if !matches!(symbol.kind, MarketKind::Spot) {
            return Err(crate::error::invalid_field(
                operation,
                "symbol",
                format!(
                    "MEXC spot public websocket only accepts spot symbols, got `{}`",
                    symbol.kind
                ),
            ));
        }
        Ok(symbol.venue_symbol.to_ascii_uppercase())
    }

    fn limit_depth(depth: Option<u16>, operation: &'static str) -> Result<u16> {
        match depth.unwrap_or(20) {
            supported @ (5 | 10 | 20) => Ok(supported),
            other => Err(crate::error::invalid_field(
                operation,
                "depth",
                format!("MEXC spot limit depth streams support 5, 10, or 20 levels, got {other}"),
            )),
        }
    }

    fn validate_delta_interval(
        max_update_interval: Option<Duration>,
        operation: &'static str,
    ) -> Result<()> {
        match max_update_interval {
            Some(interval) if interval < Duration::from_millis(10) => {
                Err(crate::error::invalid_field(
                    operation,
                    "max_update_interval",
                    "MEXC spot aggregate depth streams do not support intervals below 10ms",
                ))
            }
            Some(interval)
                if interval != Duration::from_millis(10)
                    && interval != Duration::from_millis(100) =>
            {
                Err(crate::error::invalid_field(
                    operation,
                    "max_update_interval",
                    "MEXC spot aggregate depth streams only support 10ms or 100ms cadences",
                ))
            }
            Some(_) | None => Ok(()),
        }
    }

    fn aggre_depth_interval(max_update_interval: Option<Duration>) -> &'static str {
        match max_update_interval {
            None => "10ms",
            Some(interval) if interval == Duration::from_millis(10) => "10ms",
            Some(interval) if interval == Duration::from_millis(100) => "100ms",
            Some(_) => unreachable!("validated aggregate depth interval must be 10ms or 100ms"),
        }
    }

    fn new(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for MexcChannel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for MexcChannel {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<MexcChannel> for String {
    fn from(value: MexcChannel) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MexcPublicStreamRoute {
    AggreDeals {
        symbol: Symbol,
        projections: Vec<TradeProjection>,
    },
    BookTicker {
        symbol: Symbol,
    },
    MiniTicker {
        symbol: Symbol,
    },
    Kline {
        symbol: Symbol,
        interval: KlineInterval,
    },
    OrderBook {
        symbol: Symbol,
    },
    OrderBookDelta {
        symbol: Symbol,
    },
}

/// Internal projection from one MEXC aggregate deals payload into mkt events.
///
/// `LastPrice`, `Trades`, and `AggTrades` use the same official protobuf
/// channel, so a route can project each upstream deal into the requested
/// unified event shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TradeProjection {
    LastPrice,
    Trade,
    AggTrade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MexcPublicStreamPlan {
    pub(super) channels: Vec<MexcChannel>,
    pub(super) routes: BTreeMap<MexcChannel, MexcPublicStreamRoute>,
}

impl MexcPublicStreamPlan {
    pub(super) fn build(subscriptions: &[Subscription], operation: &'static str) -> Result<Self> {
        MexcPublicStreamPlanBuilder::new(operation).build(subscriptions)
    }
}

struct MexcPublicStreamPlanBuilder {
    operation: &'static str,
    channels: Vec<MexcChannel>,
    routes: BTreeMap<MexcChannel, MexcPublicStreamRoute>,
}

impl MexcPublicStreamPlanBuilder {
    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            channels: Vec::new(),
            routes: BTreeMap::new(),
        }
    }

    fn build(mut self, subscriptions: &[Subscription]) -> Result<MexcPublicStreamPlan> {
        if subscriptions.is_empty() {
            return Err(crate::error::invalid_field(
                self.operation,
                "subscriptions",
                "at least one public subscription is required",
            ));
        }

        for subscription in subscriptions {
            let (channel, route) = self.route_for_subscription(subscription)?;
            self.insert_route(channel, route)?;
        }

        if self.channels.len() > MAX_SUBSCRIPTIONS_PER_CONNECTION {
            return Err(crate::error::invalid_field(
                self.operation,
                "subscriptions",
                format!(
                    "MEXC spot websocket supports at most {MAX_SUBSCRIPTIONS_PER_CONNECTION} channels per connection, got {}",
                    self.channels.len()
                ),
            ));
        }

        Ok(MexcPublicStreamPlan {
            channels: self.channels,
            routes: self.routes,
        })
    }

    fn route_for_subscription(
        &self,
        subscription: &Subscription,
    ) -> Result<(MexcChannel, MexcPublicStreamRoute)> {
        match subscription {
            Subscription::LastPrice(symbol) => Ok((
                MexcChannel::aggre_deals(symbol, self.operation)?,
                self.aggre_deals_route(symbol, TradeProjection::LastPrice),
            )),
            Subscription::Trades(symbol) => Ok((
                MexcChannel::aggre_deals(symbol, self.operation)?,
                self.aggre_deals_route(symbol, TradeProjection::Trade),
            )),
            Subscription::AggTrades(symbol) => Ok((
                MexcChannel::aggre_deals(symbol, self.operation)?,
                self.aggre_deals_route(symbol, TradeProjection::AggTrade),
            )),
            Subscription::BookTicker(symbol) => Ok((
                MexcChannel::book_ticker(symbol, self.operation)?,
                MexcPublicStreamRoute::BookTicker {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::MiniTicker(symbol) => Ok((
                MexcChannel::mini_ticker(symbol, self.operation)?,
                MexcPublicStreamRoute::MiniTicker {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::Klines(request) => Ok((
                MexcChannel::kline(&request.symbol, request.interval, self.operation)?,
                MexcPublicStreamRoute::Kline {
                    symbol: request.symbol.clone(),
                    interval: request.interval,
                },
            )),
            Subscription::OrderBook { symbol, depth } => Ok((
                MexcChannel::order_book(symbol, *depth, self.operation)?,
                MexcPublicStreamRoute::OrderBook {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::OrderBookDeltas {
                symbol,
                max_update_interval,
            } => Ok((
                MexcChannel::order_book_delta(symbol, *max_update_interval, self.operation)?,
                MexcPublicStreamRoute::OrderBookDelta {
                    symbol: symbol.clone(),
                },
            )),
            Subscription::BlockTrades(_) => self.unsupported("BlockTrades"),
            Subscription::AveragePrice(_) => self.unsupported("AveragePrice"),
            _ => self.unsupported("unknown future subscription"),
        }
    }

    fn aggre_deals_route(
        &self,
        symbol: &Symbol,
        projection: TradeProjection,
    ) -> MexcPublicStreamRoute {
        MexcPublicStreamRoute::AggreDeals {
            symbol: symbol.clone(),
            projections: vec![projection],
        }
    }

    fn insert_route(&mut self, channel: MexcChannel, route: MexcPublicStreamRoute) -> Result<()> {
        match self.routes.entry(channel.clone()) {
            Entry::Vacant(entry) => {
                self.channels.push(channel);
                entry.insert(route);
                Ok(())
            }
            Entry::Occupied(entry) if entry.get() == &route => Ok(()),
            Entry::Occupied(mut entry) => {
                Self::try_merge_route(entry.get_mut(), route).ok_or_else(|| {
                    crate::error::invalid_field(
                        self.operation,
                        "subscriptions",
                        "conflicting MEXC public stream routes",
                    )
                })
            }
        }
    }

    fn try_merge_route(
        existing: &mut MexcPublicStreamRoute,
        incoming: MexcPublicStreamRoute,
    ) -> Option<()> {
        match (existing, incoming) {
            (
                MexcPublicStreamRoute::AggreDeals {
                    symbol: existing_symbol,
                    projections,
                },
                MexcPublicStreamRoute::AggreDeals {
                    symbol: incoming_symbol,
                    projections: incoming_projections,
                },
            ) if *existing_symbol == incoming_symbol => {
                for projection in incoming_projections {
                    if !projections.contains(&projection) {
                        projections.push(projection);
                    }
                }
                Some(())
            }
            _ => None,
        }
    }

    fn unsupported<T>(&self, variant: &'static str) -> Result<T> {
        Err(crate::error::invalid_field(
            self.operation,
            "subscriptions",
            format!("unsupported MEXC public stream subscription: {variant}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mkt_core::Subscription;
    use mkt_types::{DerivativeKind, KlineInterval, KlineRequest, SettlementMode, Symbol};

    use super::*;

    const OPERATION: &str = "test.websocket";

    #[test]
    fn public_stream_plan_maps_supported_subscriptions_to_mexc_channels() {
        let plan = MexcPublicStreamPlan::build(
            &[
                Subscription::LastPrice(Symbol::spot("XRPUSDT")),
                Subscription::Trades(Symbol::spot("BNBUSDT")),
                Subscription::AggTrades(Symbol::spot("ADAUSDT")),
                Subscription::BookTicker(Symbol::spot("LTCUSDT")),
                Subscription::MiniTicker(Symbol::spot("DOGEUSDT")),
                Subscription::Klines(
                    KlineRequest::builder()
                        .symbol(Symbol::spot("BTCUSDT"))
                        .interval(KlineInterval::M1)
                        .build()
                        .expect("test kline request should build"),
                ),
                Subscription::OrderBook {
                    symbol: Symbol::spot("ETHUSDT"),
                    depth: Some(10),
                },
                Subscription::OrderBookDeltas {
                    symbol: Symbol::spot("SOLUSDT"),
                    max_update_interval: Some(Duration::from_millis(100)),
                },
            ],
            OPERATION,
        )
        .expect("supported subscriptions should build");

        assert_eq!(
            channel_names(&plan),
            vec![
                "spot@public.aggre.deals.v3.api.pb@100ms@XRPUSDT",
                "spot@public.aggre.deals.v3.api.pb@100ms@BNBUSDT",
                "spot@public.aggre.deals.v3.api.pb@100ms@ADAUSDT",
                "spot@public.aggre.bookTicker.v3.api.pb@100ms@LTCUSDT",
                "spot@public.miniTicker.v3.api.pb@DOGEUSDT",
                "spot@public.kline.v3.api.pb@BTCUSDT@Min1",
                "spot@public.limit.depth.v3.api.pb@ETHUSDT@10",
                "spot@public.aggre.depth.v3.api.pb@100ms@SOLUSDT",
            ]
        );
        assert!(matches!(
            plan.routes.get("spot@public.kline.v3.api.pb@BTCUSDT@Min1"),
            Some(MexcPublicStreamRoute::Kline {
                symbol,
                interval: KlineInterval::M1,
            }) if *symbol == Symbol::spot("BTCUSDT")
        ));
    }

    #[test]
    fn duplicate_subscriptions_share_one_channel() {
        let request = KlineRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .interval(KlineInterval::M1)
            .build()
            .expect("test kline request should build");
        let plan = MexcPublicStreamPlan::build(
            &[
                Subscription::Klines(request.clone()),
                Subscription::Klines(request),
            ],
            OPERATION,
        )
        .expect("duplicate subscriptions should merge");

        assert_eq!(
            channel_names(&plan),
            vec!["spot@public.kline.v3.api.pb@BTCUSDT@Min1"]
        );
    }

    #[test]
    fn trade_subscriptions_share_one_aggre_deals_channel() {
        let plan = MexcPublicStreamPlan::build(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::Trades(Symbol::spot("BTCUSDT")),
                Subscription::AggTrades(Symbol::spot("BTCUSDT")),
            ],
            OPERATION,
        )
        .expect("shared aggregate deals subscriptions should build");

        assert_eq!(
            channel_names(&plan),
            vec!["spot@public.aggre.deals.v3.api.pb@100ms@BTCUSDT"]
        );
        assert!(matches!(
            plan.routes.get("spot@public.aggre.deals.v3.api.pb@100ms@BTCUSDT"),
            Some(MexcPublicStreamRoute::AggreDeals {
                symbol,
                projections,
            }) if *symbol == Symbol::spot("BTCUSDT")
                && projections == &vec![
                    TradeProjection::LastPrice,
                    TradeProjection::Trade,
                    TradeProjection::AggTrade,
                ]
        ));
    }

    #[test]
    fn more_than_thirty_unique_channels_are_rejected() {
        let subscriptions = (0..31)
            .map(|index| Subscription::BookTicker(Symbol::spot(format!("T{index}USDT"))))
            .collect::<Vec<_>>();

        let err = MexcPublicStreamPlan::build(subscriptions.as_slice(), OPERATION)
            .expect_err("MEXC rejects more than 30 channels per websocket connection");

        assert!(err.to_string().contains("at most 30 channels"));
    }

    #[test]
    fn mini_ticker_uses_symbol_scoped_official_topic() {
        let plan = MexcPublicStreamPlan::build(
            &[Subscription::MiniTicker(Symbol::spot("BTCUSDT"))],
            OPERATION,
        )
        .expect("MEXC mini ticker subscription should build");

        assert_eq!(
            channel_names(&plan),
            vec!["spot@public.miniTicker.v3.api.pb@BTCUSDT"]
        );
        assert!(matches!(
            plan.routes.get("spot@public.miniTicker.v3.api.pb@BTCUSDT"),
            Some(MexcPublicStreamRoute::MiniTicker { symbol })
                if *symbol == Symbol::spot("BTCUSDT")
        ));
    }

    #[test]
    fn unsupported_subscriptions_are_explicitly_rejected() {
        let err = MexcPublicStreamPlan::build(
            &[Subscription::AveragePrice(Symbol::spot("BTCUSDT"))],
            OPERATION,
        )
        .expect_err("MEXC average price lacks an official public topic");

        assert!(err
            .to_string()
            .contains("unsupported MEXC public stream subscription: AveragePrice"));
    }

    #[test]
    fn non_spot_symbols_and_unsupported_options_are_rejected() {
        let non_spot = MexcPublicStreamPlan::build(
            &[Subscription::Klines(
                KlineRequest::builder()
                    .symbol(Symbol::derivative(
                        DerivativeKind::perpetual(SettlementMode::Linear),
                        "BTCUSDT",
                    ))
                    .interval(KlineInterval::M1)
                    .build()
                    .expect("test kline request should build"),
            )],
            OPERATION,
        );
        assert!(non_spot.is_err());

        let unsupported_depth = MexcPublicStreamPlan::build(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: Some(50),
            }],
            OPERATION,
        );
        assert!(unsupported_depth.is_err());

        let unsupported_interval = MexcPublicStreamPlan::build(
            &[Subscription::OrderBookDeltas {
                symbol: Symbol::spot("BTCUSDT"),
                max_update_interval: Some(Duration::from_millis(1000)),
            }],
            OPERATION,
        );
        assert!(unsupported_interval.is_err());
    }

    #[test]
    fn default_order_book_depth_uses_mexc_max_limit_depth() {
        let plan = MexcPublicStreamPlan::build(
            &[Subscription::OrderBook {
                symbol: Symbol::spot("BTCUSDT"),
                depth: None,
            }],
            OPERATION,
        )
        .expect("default depth should build");

        assert_eq!(
            channel_names(&plan),
            vec!["spot@public.limit.depth.v3.api.pb@BTCUSDT@20"]
        );
    }

    #[test]
    fn order_book_delta_channels_use_official_aggre_depth_topics() {
        let default_interval = MexcPublicStreamPlan::build(
            &[Subscription::OrderBookDeltas {
                symbol: Symbol::spot("BTCUSDT"),
                max_update_interval: None,
            }],
            OPERATION,
        )
        .expect("default aggregate depth interval should build");
        assert_eq!(
            channel_names(&default_interval),
            vec!["spot@public.aggre.depth.v3.api.pb@10ms@BTCUSDT"]
        );

        let fast_interval = MexcPublicStreamPlan::build(
            &[Subscription::OrderBookDeltas {
                symbol: Symbol::spot("ETHUSDT"),
                max_update_interval: Some(Duration::from_millis(10)),
            }],
            OPERATION,
        )
        .expect("10ms aggregate depth interval should build");
        assert_eq!(
            channel_names(&fast_interval),
            vec!["spot@public.aggre.depth.v3.api.pb@10ms@ETHUSDT"]
        );

        let slow_interval = MexcPublicStreamPlan::build(
            &[Subscription::OrderBookDeltas {
                symbol: Symbol::spot("SOLUSDT"),
                max_update_interval: Some(Duration::from_millis(100)),
            }],
            OPERATION,
        )
        .expect("100ms aggregate depth interval should build");
        assert_eq!(
            channel_names(&slow_interval),
            vec!["spot@public.aggre.depth.v3.api.pb@100ms@SOLUSDT"]
        );
        assert!(matches!(
            slow_interval
                .routes
                .get("spot@public.aggre.depth.v3.api.pb@100ms@SOLUSDT"),
            Some(MexcPublicStreamRoute::OrderBookDelta { symbol })
                if *symbol == Symbol::spot("SOLUSDT")
        ));
    }

    fn channel_names(plan: &MexcPublicStreamPlan) -> Vec<&str> {
        plan.channels.iter().map(MexcChannel::as_ref).collect()
    }
}
