use std::collections::BTreeMap;

use mkt_core::{MarketDataEvent, RawPayload, Result};
use mkt_types::{ExchangeId, KnownExchange};
use prost::Message as _;
use serde::Deserialize;

use super::plan::{MexcChannel, MexcPublicStreamRoute, TradeProjection};
use crate::{
    convert,
    protobuf::{push_data_v3_api_wrapper::Body, PushDataV3ApiWrapper},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MexcWsFrame {
    Text(String),
    Binary(Vec<u8>),
}

pub(super) fn market_data_events_from_ws_frame(
    frame: MexcWsFrame,
    routes: &BTreeMap<MexcChannel, MexcPublicStreamRoute>,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    match frame {
        MexcWsFrame::Text(raw) => text_frame_events(raw, operation),
        MexcWsFrame::Binary(raw) => binary_frame_events(raw, routes, operation),
    }
}

fn text_frame_events(raw: String, operation: &'static str) -> Result<Vec<MarketDataEvent>> {
    let message: MexcControlEnvelope = serde_json::from_str(raw.as_str()).map_err(|err| {
        crate::error::decode_error(operation, format!("invalid websocket JSON: {err}"))
    })?;

    match MexcControlMessage::from(message) {
        MexcControlMessage::Pong | MexcControlMessage::SubscriptionAck => Ok(Vec::new()),
        MexcControlMessage::Error { message } => Err(crate::error::invalid_field(
            operation,
            "control",
            message.unwrap_or_else(|| "MEXC websocket control error".to_owned()),
        )),
        MexcControlMessage::Unknown => Ok(vec![raw_text_event(raw)]),
    }
}

fn binary_frame_events(
    raw: Vec<u8>,
    routes: &BTreeMap<MexcChannel, MexcPublicStreamRoute>,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    let wrapper = PushDataV3ApiWrapper::decode(raw.as_slice()).map_err(|err| {
        crate::error::decode_error(operation, format!("invalid MEXC protobuf wrapper: {err}"))
    })?;

    let Some(route) = routes.get(wrapper.channel.as_str()) else {
        return Ok(vec![raw_binary_event(raw)]);
    };

    let events = match (route, wrapper.body) {
        (
            MexcPublicStreamRoute::AggreDeals {
                symbol,
                projections,
            },
            Some(Body::PublicAggreDeals(body)),
        ) => trade_events_from_aggre_deals(symbol, projections, body, operation)?,
        (MexcPublicStreamRoute::BookTicker { symbol }, Some(Body::PublicAggreBookTicker(body))) => {
            vec![MarketDataEvent::BookTicker(
                convert::stream::book_ticker_from_aggre_book_ticker(
                    symbol.clone(),
                    body,
                    operation,
                )?,
            )]
        }
        (MexcPublicStreamRoute::MiniTicker { symbol }, Some(Body::PublicMiniTicker(body))) => {
            vec![MarketDataEvent::MiniTicker(
                convert::stream::mini_ticker_from_proto(symbol, body, operation)?,
            )]
        }
        (MexcPublicStreamRoute::MiniTicker { symbol }, Some(Body::PublicMiniTickers(body))) => {
            convert::stream::mini_tickers_from_batch(symbol, body, operation)?
                .into_iter()
                .map(MarketDataEvent::MiniTicker)
                .collect()
        }
        (MexcPublicStreamRoute::Kline { symbol, interval }, Some(Body::PublicSpotKline(body))) => {
            vec![MarketDataEvent::Kline(convert::stream::kline_from_proto(
                symbol.clone(),
                *interval,
                body,
                operation,
            )?)]
        }
        (MexcPublicStreamRoute::OrderBook { symbol }, Some(Body::PublicLimitDepths(body))) => {
            vec![MarketDataEvent::OrderBook(
                convert::stream::order_book_from_limit_depths(symbol.clone(), body, operation)?,
            )]
        }
        (
            MexcPublicStreamRoute::OrderBookDelta { symbol },
            Some(Body::PublicIncreaseDepths(body)),
        ) => {
            vec![MarketDataEvent::OrderBookDelta(
                convert::stream::order_book_delta_from_increase_depths(
                    symbol.clone(),
                    body,
                    operation,
                )?,
            )]
        }
        (MexcPublicStreamRoute::OrderBookDelta { symbol }, Some(Body::PublicAggreDepths(body))) => {
            vec![MarketDataEvent::OrderBookDelta(
                convert::stream::order_book_delta_from_aggre_depths(
                    symbol.clone(),
                    body,
                    operation,
                )?,
            )]
        }
        (_, Some(_)) => {
            return Err(crate::error::decode_error(
                operation,
                format!(
                    "MEXC protobuf body did not match subscribed channel `{}`",
                    wrapper.channel
                ),
            ));
        }
        (_, None) => {
            return Err(crate::error::decode_error(
                operation,
                format!(
                    "MEXC protobuf wrapper for `{}` had no body",
                    wrapper.channel
                ),
            ));
        }
    };

    Ok(events)
}

fn trade_events_from_aggre_deals(
    symbol: &mkt_types::Symbol,
    projections: &[TradeProjection],
    body: crate::protobuf::PublicAggreDealsV3Api,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    let mut events = Vec::new();

    for projection in projections {
        match projection {
            TradeProjection::LastPrice => events.extend(
                convert::stream::last_prices_from_aggre_deals(
                    symbol.clone(),
                    body.clone(),
                    operation,
                )?
                .into_iter()
                .map(MarketDataEvent::LastPrice),
            ),
            TradeProjection::Trade => events.extend(
                convert::stream::trades_from_aggre_deals(symbol.clone(), body.clone(), operation)?
                    .into_iter()
                    .map(MarketDataEvent::Trade),
            ),
            TradeProjection::AggTrade => events.extend(
                convert::stream::agg_trades_from_aggre_deals(
                    symbol.clone(),
                    body.clone(),
                    operation,
                )?
                .into_iter()
                .map(MarketDataEvent::AggTrade),
            ),
        }
    }

    Ok(events)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MexcControlEnvelope {
    Event {
        #[serde(default)]
        code: Option<i64>,
        #[serde(default)]
        msg: Option<String>,
        #[serde(default)]
        id: Option<serde_json::Value>,
    },
    Raw(serde_json::Value),
}

enum MexcControlMessage {
    Pong,
    SubscriptionAck,
    Error { message: Option<String> },
    Unknown,
}

impl From<MexcControlEnvelope> for MexcControlMessage {
    fn from(value: MexcControlEnvelope) -> Self {
        match value {
            MexcControlEnvelope::Event { code, msg, id } => match (code, msg.as_deref(), id) {
                (Some(0), _, _) => Self::SubscriptionAck,
                (Some(code), message, _) if code != 0 => Self::Error {
                    message: message.map(ToOwned::to_owned),
                },
                (_, Some("PONG"), _) => Self::Pong,
                (_, Some("pong"), _) => Self::Pong,
                (_, _, Some(_)) => Self::SubscriptionAck,
                _ => Self::Unknown,
            },
            MexcControlEnvelope::Raw(value) => {
                if value
                    .get("msg")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|message| message.eq_ignore_ascii_case("pong"))
                {
                    Self::Pong
                } else {
                    Self::Unknown
                }
            }
        }
    }
}

fn raw_text_event(raw: String) -> MarketDataEvent {
    MarketDataEvent::Raw {
        exchange_id: ExchangeId::from(KnownExchange::Mexc),
        payload: RawPayload::Text(raw),
    }
}

fn raw_binary_event(raw: Vec<u8>) -> MarketDataEvent {
    MarketDataEvent::Raw {
        exchange_id: ExchangeId::from(KnownExchange::Mexc),
        payload: RawPayload::Binary(raw),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use mkt_core::{RawPayload, Subscription};
    use mkt_types::{BookTicker, LastPrice, MiniTicker, Symbol, Trade, TradeSide};
    use prost::Message as _;
    use rust_decimal::Decimal;

    use super::*;
    use crate::protobuf::{
        PublicAggreBookTickerV3Api, PublicAggreDealsV3Api, PublicAggreDealsV3ApiItem,
        PublicDealsV3Api, PublicMiniTickerV3Api, PublicMiniTickersV3Api,
    };

    const OPERATION: &str = "test.websocket";
    use super::super::plan::MexcPublicStreamPlan;

    #[test]
    fn aggre_deals_project_to_requested_trade_events() {
        let routes = plan_routes([
            Subscription::LastPrice(Symbol::spot("BTCUSDT")),
            Subscription::Trades(Symbol::spot("BTCUSDT")),
            Subscription::AggTrades(Symbol::spot("BTCUSDT")),
        ]);
        let channel = "spot@public.aggre.deals.v3.api.pb@100ms@BTCUSDT";

        let events = decode(
            wrapper(
                channel,
                Some(Body::PublicAggreDeals(PublicAggreDealsV3Api {
                    deals: vec![PublicAggreDealsV3ApiItem {
                        price: "100.25".to_owned(),
                        quantity: "0.5".to_owned(),
                        trade_type: 1,
                        time: 1_672_515_782_136,
                    }],
                    event_type: "deals".to_owned(),
                })),
            ),
            &routes,
        );

        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            MarketDataEvent::LastPrice(LastPrice { symbol, price, .. })
                if *symbol == Symbol::spot("BTCUSDT")
                    && *price == decimal("100.25")
        ));
        assert!(matches!(
            &events[1],
            MarketDataEvent::Trade(Trade { symbol, id, side: TradeSide::Buy, timestamp, .. })
                if *symbol == Symbol::spot("BTCUSDT")
                    && id.is_none()
                    && timestamp.unix_timestamp_nanos() == 1_672_515_782_136_i128 * 1_000_000
        ));
        assert!(matches!(
            &events[2],
            MarketDataEvent::AggTrade(trade)
                if trade.symbol == Symbol::spot("BTCUSDT")
                    && trade.id.is_none()
                    && trade.side == TradeSide::Buy
                    && trade.price == decimal("100.25")
        ));
    }

    #[test]
    fn aggre_deals_project_to_agg_trade_events() {
        let routes = plan_routes([Subscription::AggTrades(Symbol::spot("ETHUSDT"))]);
        let channel = "spot@public.aggre.deals.v3.api.pb@100ms@ETHUSDT";

        let events = decode(
            wrapper(
                channel,
                Some(Body::PublicAggreDeals(PublicAggreDealsV3Api {
                    deals: vec![PublicAggreDealsV3ApiItem {
                        price: "2500.5".to_owned(),
                        quantity: "2".to_owned(),
                        trade_type: 2,
                        time: 1_672_515_782_000,
                    }],
                    event_type: "aggreDeals".to_owned(),
                })),
            ),
            &routes,
        );

        assert!(matches!(
            &events[0],
            MarketDataEvent::AggTrade(trade)
                if trade.symbol == Symbol::spot("ETHUSDT")
                    && trade.id.is_none()
                    && trade.side == TradeSide::Sell
                    && trade.price == decimal("2500.5")
        ));
    }

    #[test]
    fn book_ticker_projects_to_book_ticker_event() {
        let routes = plan_routes([Subscription::BookTicker(Symbol::spot("XRPUSDT"))]);
        let channel = "spot@public.aggre.bookTicker.v3.api.pb@100ms@XRPUSDT";

        let events = decode(
            wrapper(
                channel,
                Some(Body::PublicAggreBookTicker(PublicAggreBookTickerV3Api {
                    bid_price: "0.25".to_owned(),
                    bid_quantity: "200".to_owned(),
                    ask_price: "0.26".to_owned(),
                    ask_quantity: "300".to_owned(),
                })),
            ),
            &routes,
        );

        assert!(matches!(
            &events[0],
            MarketDataEvent::BookTicker(BookTicker { symbol, bid_price, ask_price, .. })
                if *symbol == Symbol::spot("XRPUSDT")
                    && *bid_price == decimal("0.25")
                    && *ask_price == decimal("0.26")
        ));
    }

    #[test]
    fn mini_ticker_projects_to_mini_ticker_event() {
        let routes = plan_routes([Subscription::MiniTicker(Symbol::spot("DOGEUSDT"))]);
        let channel = "spot@public.miniTicker.v3.api.pb@DOGEUSDT";

        let events = decode(
            wrapper(
                channel,
                Some(Body::PublicMiniTicker(PublicMiniTickerV3Api {
                    symbol: "DOGEUSDT".to_owned(),
                    price: "0.12".to_owned(),
                    rate: "0.1".to_owned(),
                    zoned_rate: "0.1".to_owned(),
                    high: "0.13".to_owned(),
                    low: "0.10".to_owned(),
                    volume: "1200.5".to_owned(),
                    quantity: "10000".to_owned(),
                    last_close_rate: "0.1".to_owned(),
                    last_close_zoned_rate: "0.1".to_owned(),
                    last_close_high: "0.13".to_owned(),
                    last_close_low: "0.10".to_owned(),
                })),
            ),
            &routes,
        );

        assert!(matches!(
            &events[0],
            MarketDataEvent::MiniTicker(MiniTicker { symbol, close, high, low, volume_base, volume_quote, .. })
                if *symbol == Symbol::spot("DOGEUSDT")
                    && *close == decimal("0.12")
                    && *high == decimal("0.13")
                    && *low == decimal("0.10")
                    && *volume_base == decimal("10000")
                    && *volume_quote == decimal("1200.5")
        ));
    }

    #[test]
    fn mini_ticker_batch_filters_to_subscribed_symbol() {
        let routes = plan_routes([Subscription::MiniTicker(Symbol::spot("DOGEUSDT"))]);
        let channel = "spot@public.miniTicker.v3.api.pb@DOGEUSDT";

        let events = decode(
            wrapper(
                channel,
                Some(Body::PublicMiniTickers(PublicMiniTickersV3Api {
                    items: vec![
                        mini_ticker_item("BTCUSDT", "100"),
                        mini_ticker_item("DOGEUSDT", "0.12"),
                    ],
                })),
            ),
            &routes,
        );

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            MarketDataEvent::MiniTicker(MiniTicker { symbol, close, .. })
                if *symbol == Symbol::spot("DOGEUSDT")
                    && *close == decimal("0.12")
        ));
    }

    #[test]
    fn unknown_binary_channel_is_preserved_as_raw() {
        let raw = encode(wrapper(
            "spot@public.deals.v3.api.pb@UNKNOWN",
            Some(Body::PublicDeals(PublicDealsV3Api {
                deals: Vec::new(),
                event_type: "deals".to_owned(),
            })),
        ));

        let events = market_data_events_from_ws_frame(
            MexcWsFrame::Binary(raw.clone()),
            &BTreeMap::new(),
            OPERATION,
        )
        .expect("unknown channel should not fail decoding");

        assert!(matches!(
            &events[0],
            MarketDataEvent::Raw {
                payload: RawPayload::Binary(payload),
                ..
            } if payload == &raw
        ));
    }

    #[test]
    fn subscribed_channel_with_wrong_body_is_decode_error() {
        let routes = plan_routes([Subscription::BookTicker(Symbol::spot("XRPUSDT"))]);
        let channel = "spot@public.aggre.bookTicker.v3.api.pb@100ms@XRPUSDT";

        let err = market_data_events_from_ws_frame(
            MexcWsFrame::Binary(encode(wrapper(
                channel,
                Some(Body::PublicDeals(PublicDealsV3Api {
                    deals: Vec::new(),
                    event_type: "deals".to_owned(),
                })),
            ))),
            &routes,
            OPERATION,
        )
        .expect_err("wrong body for subscribed channel should fail");

        assert!(err.to_string().contains("body did not match"));
    }

    fn plan_routes<const N: usize>(
        subscriptions: [Subscription; N],
    ) -> BTreeMap<MexcChannel, MexcPublicStreamRoute> {
        MexcPublicStreamPlan::build(&subscriptions, OPERATION)
            .expect("test public stream plan should build")
            .routes
    }

    fn wrapper(channel: &'static str, body: Option<Body>) -> PushDataV3ApiWrapper {
        PushDataV3ApiWrapper {
            channel: channel.to_owned(),
            body,
            symbol: None,
            symbol_id: None,
            create_time: None,
            send_time: None,
        }
    }

    fn decode(
        wrapper: PushDataV3ApiWrapper,
        routes: &BTreeMap<MexcChannel, MexcPublicStreamRoute>,
    ) -> Vec<MarketDataEvent> {
        market_data_events_from_ws_frame(MexcWsFrame::Binary(encode(wrapper)), routes, OPERATION)
            .expect("protobuf wrapper should map to events")
    }

    fn encode(wrapper: PushDataV3ApiWrapper) -> Vec<u8> {
        wrapper.encode_to_vec()
    }

    fn mini_ticker_item(symbol: &str, price: &str) -> PublicMiniTickerV3Api {
        PublicMiniTickerV3Api {
            symbol: symbol.to_owned(),
            price: price.to_owned(),
            rate: "0.1".to_owned(),
            zoned_rate: "0.1".to_owned(),
            high: price.to_owned(),
            low: price.to_owned(),
            volume: "1200.5".to_owned(),
            quantity: "10000".to_owned(),
            last_close_rate: "0.1".to_owned(),
            last_close_zoned_rate: "0.1".to_owned(),
            last_close_high: price.to_owned(),
            last_close_low: price.to_owned(),
        }
    }

    fn decimal(raw: &str) -> Decimal {
        Decimal::from_str(raw).expect("test decimal must be valid")
    }
}
