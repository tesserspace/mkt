use std::collections::BTreeMap;
use std::str::FromStr;

use mkt_core::{MarketDataEvent, RawPayload, Result, Subscription};
use mkt_types::{
    Kline, KlineInterval, LastPrice, OrderBook, OrderBookLevel, Symbol, Trade, TradeSide,
};
use rust_decimal::Decimal;
use serde_json::Value;

use super::internal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BinancePublicStreamRoute {
    LastPrice,
    OrderBook { symbol: Symbol },
    Trade,
    Kline { interval: KlineInterval },
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
                format!("{}@miniTicker", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::LastPrice,
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
            Subscription::Trades(symbol) => (
                format!("{}@trade", stream_symbol(symbol, operation)?),
                BinancePublicStreamRoute::Trade,
            ),
            Subscription::Klines(request) => (
                format!(
                    "{}@kline_{}",
                    stream_symbol(&request.symbol, operation)?,
                    kline_interval(request.interval, operation)?
                ),
                BinancePublicStreamRoute::Kline {
                    interval: request.interval,
                },
            ),
        };

        if routes.insert(stream_name.clone(), route).is_none() {
            stream_names.push(stream_name);
        }
    }

    Ok(BinancePublicStreamPlan {
        stream_names,
        routes,
    })
}

pub(crate) fn market_data_event_from_ws_text(
    raw: &str,
    routes: &BTreeMap<String, BinancePublicStreamRoute>,
    operation: &'static str,
) -> Result<MarketDataEvent> {
    let value: Value = serde_json::from_str(raw).map_err(|err| {
        crate::error::decode_error(operation, format!("invalid websocket JSON: {err}"))
    })?;
    let Some(stream_name) = value.get("stream").and_then(Value::as_str) else {
        return Ok(raw_event(raw));
    };
    let Some(payload) = value.get("data") else {
        return Ok(raw_event(raw));
    };
    let Some(route) = routes.get(stream_name) else {
        return Ok(raw_event(raw));
    };

    match route {
        BinancePublicStreamRoute::LastPrice => Ok(MarketDataEvent::LastPrice(
            last_price_from_value(payload, operation)?,
        )),
        BinancePublicStreamRoute::OrderBook { symbol } => Ok(MarketDataEvent::OrderBook(
            order_book_from_value(symbol, payload, operation)?,
        )),
        BinancePublicStreamRoute::Trade => Ok(MarketDataEvent::Trade(trade_from_value(
            payload, operation,
        )?)),
        BinancePublicStreamRoute::Kline { interval } => Ok(MarketDataEvent::Kline(
            kline_from_value(payload, *interval, operation)?,
        )),
    }
}

fn stream_symbol(symbol: &Symbol, operation: &'static str) -> Result<String> {
    let symbol = crate::convert::require_spot_symbol(symbol, operation)?;
    Ok(symbol.to_ascii_lowercase())
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

fn kline_interval(interval: KlineInterval, operation: &'static str) -> Result<&'static str> {
    match interval {
        KlineInterval::Second(1) => Ok("1s"),
        KlineInterval::Minute(1) => Ok("1m"),
        KlineInterval::Minute(3) => Ok("3m"),
        KlineInterval::Minute(5) => Ok("5m"),
        KlineInterval::Minute(15) => Ok("15m"),
        KlineInterval::Minute(30) => Ok("30m"),
        KlineInterval::Hour(1) => Ok("1h"),
        KlineInterval::Hour(2) => Ok("2h"),
        KlineInterval::Hour(4) => Ok("4h"),
        KlineInterval::Hour(6) => Ok("6h"),
        KlineInterval::Hour(8) => Ok("8h"),
        KlineInterval::Hour(12) => Ok("12h"),
        KlineInterval::Day(1) => Ok("1d"),
        KlineInterval::Day(3) => Ok("3d"),
        KlineInterval::Week(1) => Ok("1w"),
        KlineInterval::Month(1) => Ok("1M"),
        _ => Err(crate::error::invalid_field(
            operation,
            "interval",
            "unsupported Binance spot kline stream interval",
        )),
    }
}

fn last_price_from_value(value: &Value, operation: &'static str) -> Result<LastPrice> {
    let response: binance_sdk::spot::websocket_streams::MiniTickerResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid mini ticker payload: {err}"))
        })?;

    Ok(LastPrice::new(
        Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ),
        internal::parse_required_decimal(response.c, operation, "c")?,
    ))
}

fn order_book_from_value(
    symbol: &Symbol,
    value: &Value,
    operation: &'static str,
) -> Result<OrderBook> {
    let response: binance_sdk::spot::websocket_streams::PartialBookDepthResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid partial book payload: {err}"))
        })?;

    OrderBook::builder()
        .symbol(symbol.clone())
        .bids(levels_from_response(response.bids, operation, "bids")?)
        .asks(levels_from_response(response.asks, operation, "asks")?)
        .last_update_id(response.last_update_id.map(|value| value.to_string()))
        .timestamp(None)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order_book", err.to_string()))
}

fn trade_from_value(value: &Value, operation: &'static str) -> Result<Trade> {
    let response: binance_sdk::spot::websocket_streams::TradeResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid trade payload: {err}"))
        })?;

    Trade::builder()
        .symbol(Symbol::spot(
            response
                .s
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .id(response.t.map(|value| value.to_string()))
        .price(internal::parse_required_decimal(
            response.p, operation, "p",
        )?)
        .quantity(internal::parse_required_decimal(
            response.q, operation, "q",
        )?)
        .side(match response.m {
            Some(true) => TradeSide::Sell,
            Some(false) => TradeSide::Buy,
            None => return Err(crate::error::missing_field(operation, "m")),
        })
        .timestamp(internal::parse_required_unix_millis_timestamp(
            response.t_uppercase,
            operation,
            "T",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "trade", err.to_string()))
}

fn kline_from_value(
    value: &Value,
    interval: KlineInterval,
    operation: &'static str,
) -> Result<Kline> {
    let response: binance_sdk::spot::websocket_streams::KlineResponse =
        serde_json::from_value(value.clone()).map_err(|err| {
            crate::error::decode_error(operation, format!("invalid kline payload: {err}"))
        })?;
    let kline = response
        .k
        .ok_or_else(|| crate::error::missing_field(operation, "k"))?;

    Kline::builder()
        .symbol(Symbol::spot(
            kline
                .s
                .or(response.s)
                .ok_or_else(|| crate::error::missing_field(operation, "s"))?,
        ))
        .interval(interval)
        .open_time(internal::parse_required_unix_millis_timestamp(
            kline.t, operation, "k.t",
        )?)
        .close_time(internal::parse_required_unix_millis_timestamp(
            kline.t_uppercase,
            operation,
            "k.T",
        )?)
        .open(internal::parse_required_decimal(kline.o, operation, "k.o")?)
        .high(internal::parse_required_decimal(kline.h, operation, "k.h")?)
        .low(internal::parse_required_decimal(kline.l, operation, "k.l")?)
        .close(internal::parse_required_decimal(kline.c, operation, "k.c")?)
        .volume_base(internal::parse_required_decimal(kline.v, operation, "k.v")?)
        .volume_quote(Some(internal::parse_required_decimal(
            kline.q, operation, "k.q",
        )?))
        .closed(
            kline
                .x
                .ok_or_else(|| crate::error::missing_field(operation, "k.x"))?,
        )
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "kline", err.to_string()))
}

fn levels_from_response(
    levels: Option<Vec<Vec<String>>>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    levels
        .unwrap_or_default()
        .into_iter()
        .map(|level| {
            if level.len() < 2 {
                return Err(crate::error::invalid_field(
                    operation,
                    field,
                    "expected price/quantity level pair",
                ));
            }

            Ok(OrderBookLevel::new(
                Decimal::from_str(level[0].as_str()).map_err(|err| {
                    crate::error::invalid_field(operation, field, err.to_string())
                })?,
                Decimal::from_str(level[1].as_str()).map_err(|err| {
                    crate::error::invalid_field(operation, field, err.to_string())
                })?,
            ))
        })
        .collect()
}

fn raw_event(raw: &str) -> MarketDataEvent {
    MarketDataEvent::Raw {
        exchange_id: mkt_types::ExchangeId::from(mkt_types::KnownExchange::Binance),
        payload: RawPayload::Text(raw.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use mkt_types::KlineRequest;

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
                "btcusdt@miniTicker",
                "ethusdt@depth10",
                "bnbusdt@trade",
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
    fn websocket_payloads_map_to_domain_events() {
        let plan = build_public_stream_plan(
            &[
                Subscription::LastPrice(Symbol::spot("BTCUSDT")),
                Subscription::OrderBook {
                    symbol: Symbol::spot("ETHUSDT"),
                    depth: Some(5),
                },
                Subscription::Trades(Symbol::spot("BNBUSDT")),
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

        let last_price = market_data_event_from_ws_text(
            r#"{"stream":"btcusdt@miniTicker","data":{"e":"24hrMiniTicker","E":1672515782136,"s":"BTCUSDT","c":"431.50000000"}}"#,
            &plan.routes,
            OPERATION,
        )
        .expect("mini ticker should map");
        assert!(matches!(
            last_price,
            MarketDataEvent::LastPrice(LastPrice { symbol, price, .. })
                if symbol == Symbol::spot("BTCUSDT")
                    && price == Decimal::from_str("431.50000000").expect("valid decimal")
        ));

        let order_book = market_data_event_from_ws_text(
            r#"{"stream":"ethusdt@depth5","data":{"lastUpdateId":160,"bids":[["0.0024","10"]],"asks":[["0.0026","100"]]}}"#,
            &plan.routes,
            OPERATION,
        )
        .expect("partial book should map");
        assert!(matches!(
            order_book,
            MarketDataEvent::OrderBook(OrderBook { symbol, bids, asks, last_update_id, .. })
                if symbol == Symbol::spot("ETHUSDT")
                    && bids.len() == 1
                    && asks.len() == 1
                    && last_update_id.as_deref() == Some("160")
        ));

        let trade = market_data_event_from_ws_text(
            r#"{"stream":"bnbusdt@trade","data":{"e":"trade","E":1672515782136,"s":"BNBUSDT","t":12345,"p":"0.001","q":"100","T":1672515782136,"m":true,"M":true}}"#,
            &plan.routes,
            OPERATION,
        )
        .expect("trade should map");
        assert!(matches!(
            trade,
            MarketDataEvent::Trade(Trade { symbol, id, side: TradeSide::Sell, .. })
                if symbol == Symbol::spot("BNBUSDT") && id.as_deref() == Some("12345")
        ));

        let kline = market_data_event_from_ws_text(
            r#"{"stream":"solusdt@kline_1m","data":{"e":"kline","E":1672515782136,"s":"SOLUSDT","k":{"t":1672515780000,"T":1672515839999,"s":"SOLUSDT","i":"1m","f":100,"L":200,"o":"0.0010","c":"0.0020","h":"0.0025","l":"0.0015","v":"1000","n":100,"x":false,"q":"1.0000","V":"500","Q":"0.500","B":"0"}}}"#,
            &plan.routes,
            OPERATION,
        )
        .expect("kline should map");
        assert!(matches!(
            kline,
            MarketDataEvent::Kline(Kline { symbol, interval: KlineInterval::M1, closed: false, .. })
                if symbol == Symbol::spot("SOLUSDT")
        ));
    }

    #[test]
    fn non_stream_messages_are_preserved_as_raw_events() {
        let event = market_data_event_from_ws_text(
            r#"{"result":null,"id":"subscribe-1"}"#,
            &BTreeMap::new(),
            OPERATION,
        )
        .expect("control messages should not fail decoding");

        assert!(matches!(
            event,
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
}
