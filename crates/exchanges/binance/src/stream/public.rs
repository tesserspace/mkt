use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use binance_sdk::models::{WebsocketEvent, WebsocketStreamsConnectConfig};
use mkt_core::{EventStream, MarketDataEvent, PublicStream, RawPayload, Result, Subscription};
use mkt_types::{ExchangeId, KnownExchange, LastPrice};
use serde_json::Value;
use tokio::{
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    task::JoinHandle,
};

use super::plan::{build_public_stream_plan, BinancePublicStreamRoute, TradeOutput};
use crate::{convert, error, BinanceInner};

const SUBSCRIBE_PUBLIC_OPERATION: &str = "spot.public_stream.subscribe";
const PUBLIC_STREAM_EVENT_OPERATION: &str = "spot.public_stream.event";
const EVENT_BUFFER_CAPACITY: usize = 1024;

pub(crate) struct BinancePublicStream {
    inner: Arc<BinanceInner>,
}

impl BinancePublicStream {
    pub(crate) fn new(inner: Arc<BinanceInner>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl PublicStream for BinancePublicStream {
    async fn subscribe_public(
        &self,
        subscriptions: Vec<Subscription>,
    ) -> Result<Box<dyn EventStream>> {
        let plan = build_public_stream_plan(subscriptions.as_slice(), SUBSCRIBE_PUBLIC_OPERATION)?;
        let websocket_streams = self
            .inner
            .spot_ws_streams
            .connect_with_config(WebsocketStreamsConnectConfig {
                streams: plan.stream_names,
                mode: None,
            })
            .await
            .map_err(|err| error::map_request_error(SUBSCRIBE_PUBLIC_OPERATION, err))?;
        let routes = Arc::new(plan.routes);
        let (tx, rx) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        let overflowed = Arc::new(AtomicBool::new(false));
        let terminal_sent = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let event_subscription = websocket_streams.subscribe_on_ws_events({
            let routes = Arc::clone(&routes);
            let overflowed = Arc::clone(&overflowed);
            let terminal_sent = Arc::clone(&terminal_sent);
            move |event| {
                if overflowed.load(Ordering::Acquire) || terminal_sent.load(Ordering::Acquire) {
                    return;
                }

                let messages = match event {
                    WebsocketEvent::Message(raw) => {
                        match market_data_events_from_ws_text(
                            raw.as_str(),
                            routes.as_ref(),
                            PUBLIC_STREAM_EVENT_OPERATION,
                        ) {
                            Ok(events) => events
                                .into_iter()
                                .map(|event| PublicStreamMessage::Event(Ok(event)))
                                .collect(),
                            Err(err) => vec![PublicStreamMessage::Event(Err(err))],
                        }
                    }
                    WebsocketEvent::Error(message) => vec![PublicStreamMessage::Terminal(Err(
                        error::websocket_error(PUBLIC_STREAM_EVENT_OPERATION, message),
                    ))],
                    WebsocketEvent::Close(1000, _) => {
                        vec![PublicStreamMessage::Terminal(Ok(()))]
                    }
                    WebsocketEvent::Close(code, reason) => {
                        vec![PublicStreamMessage::Terminal(Err(error::websocket_error(
                            PUBLIC_STREAM_EVENT_OPERATION,
                            format!("Binance public websocket closed with code {code}: {reason}"),
                        )))]
                    }
                    WebsocketEvent::Open | WebsocketEvent::Ping | WebsocketEvent::Pong => {
                        Vec::new()
                    }
                };

                for message in messages {
                    if overflowed.load(Ordering::Acquire) || terminal_sent.load(Ordering::Acquire) {
                        return;
                    }

                    let is_terminal = matches!(message, PublicStreamMessage::Terminal(_));

                    match tx.try_send(message) {
                        Ok(()) => {
                            if is_terminal {
                                terminal_sent.store(true, Ordering::Release);
                            }
                        }
                        Err(TrySendError::Closed(_)) => {}
                        Err(TrySendError::Full(_)) => {
                            overflowed.store(true, Ordering::Release);
                        }
                    }
                }
            }
        });

        let shutdown_task = tokio::spawn(async move {
            let _ = shutdown_rx.await;
            event_subscription.unsubscribe();
            let _ = websocket_streams.disconnect().await;
        });

        Ok(Box::new(BinancePublicEventStream {
            receiver: rx,
            shutdown_tx: Some(shutdown_tx),
            shutdown_task: Some(shutdown_task),
            overflowed,
            terminated: false,
        }))
    }
}

enum PublicStreamMessage {
    Event(Result<MarketDataEvent>),
    Terminal(Result<()>),
}

struct BinancePublicEventStream {
    receiver: mpsc::Receiver<PublicStreamMessage>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    shutdown_task: Option<JoinHandle<()>>,
    overflowed: Arc<AtomicBool>,
    terminated: bool,
}

impl BinancePublicEventStream {
    async fn shutdown(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(shutdown_task) = self.shutdown_task.take() {
            let _ = shutdown_task.await;
        }
    }

    async fn terminate_with(&mut self, result: Result<()>) -> Result<Option<MarketDataEvent>> {
        self.terminated = true;
        self.shutdown().await;
        result.map(|()| None)
    }
}

#[async_trait]
impl EventStream for BinancePublicEventStream {
    async fn next(&mut self) -> Result<Option<MarketDataEvent>> {
        if self.terminated {
            return Ok(None);
        }

        if self.overflowed.load(Ordering::Acquire) {
            return self
                .terminate_with(Err(error::websocket_error(
                    PUBLIC_STREAM_EVENT_OPERATION,
                    format!(
                        "Binance public websocket event buffer overflowed after {EVENT_BUFFER_CAPACITY} queued events"
                    ),
                )))
                .await;
        }

        match self.receiver.recv().await {
            Some(PublicStreamMessage::Event(Ok(event))) => Ok(Some(event)),
            Some(PublicStreamMessage::Event(Err(err))) => Err(err),
            Some(PublicStreamMessage::Terminal(result)) => self.terminate_with(result).await,
            None => self.terminate_with(Ok(())).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        self.terminated = true;
        self.shutdown().await;
        Ok(())
    }
}

impl Drop for BinancePublicEventStream {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

fn market_data_events_from_ws_text(
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
            trade_events_from_payload(payload, outputs, operation)
        }
        BinancePublicStreamRoute::AggTrade => Ok(vec![MarketDataEvent::AggTrade(
            convert::stream::agg_trade_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::BlockTrade => Ok(vec![MarketDataEvent::BlockTrade(
            convert::stream::block_trade_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::BookTicker => Ok(vec![MarketDataEvent::BookTicker(
            convert::stream::book_ticker_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::OrderBook { symbol } => Ok(vec![MarketDataEvent::OrderBook(
            convert::stream::order_book_from_value(symbol, payload, operation)?,
        )]),
        BinancePublicStreamRoute::OrderBookDelta => Ok(vec![MarketDataEvent::OrderBookDelta(
            convert::stream::order_book_delta_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::AveragePrice => Ok(vec![MarketDataEvent::AveragePrice(
            convert::stream::average_price_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::MiniTicker => Ok(vec![MarketDataEvent::MiniTicker(
            convert::stream::mini_ticker_from_value(payload, operation)?,
        )]),
        BinancePublicStreamRoute::Kline { interval } => Ok(vec![MarketDataEvent::Kline(
            convert::stream::kline_from_value(payload, *interval, operation)?,
        )]),
    }
}

fn trade_events_from_payload(
    payload: &Value,
    outputs: &std::collections::BTreeSet<TradeOutput>,
    operation: &'static str,
) -> Result<Vec<MarketDataEvent>> {
    let trade = convert::stream::trade_from_value(payload, operation)?;
    let mut events = Vec::with_capacity(outputs.len());

    if outputs.contains(&TradeOutput::LastPrice) {
        events.push(MarketDataEvent::LastPrice(LastPrice::new(
            trade.symbol.clone(),
            trade.price,
        )));
    }

    if outputs.contains(&TradeOutput::Trade) {
        events.push(MarketDataEvent::Trade(trade));
    }

    Ok(events)
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

    use mkt_core::{ErrorKind, EventStream, RawPayload, Subscription};
    use mkt_types::{
        AggTrade, AveragePrice, BlockTrade, BookDepthUpdateSpeed, BookTicker, Kline, KlineInterval,
        KlineRequest, MiniTicker, OrderBook, OrderBookDelta, Symbol, Trade, TradeSide,
    };
    use rust_decimal::Decimal;

    use super::*;

    const OPERATION: &str = "test.websocket";

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

    #[tokio::test]
    async fn event_stream_reports_buffer_overflow_as_terminal_error() {
        let (_tx, rx) = mpsc::channel(1);
        let overflowed = Arc::new(AtomicBool::new(true));
        let mut stream = BinancePublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            shutdown_task: None,
            overflowed,
            terminated: false,
        };

        let err = stream
            .next()
            .await
            .expect_err("overflow should be reported before reading more events");

        assert_eq!(err.kind(), ErrorKind::Transport);
        assert_eq!(err.operation(), Some(PUBLIC_STREAM_EVENT_OPERATION));
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[tokio::test]
    async fn explicit_close_waits_for_shutdown_task() {
        let (_tx, rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let shutdown_task = tokio::spawn(async move {
            shutdown_rx
                .await
                .expect("explicit close should notify shutdown task");
        });
        let mut stream = BinancePublicEventStream {
            receiver: rx,
            shutdown_tx: Some(shutdown_tx),
            shutdown_task: Some(shutdown_task),
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        stream
            .close()
            .await
            .expect("explicit close should finish shutdown task");

        assert!(stream.shutdown_tx.is_none());
        assert!(stream.shutdown_task.is_none());
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[tokio::test]
    async fn event_stream_treats_sdk_error_as_terminal() {
        let (tx, rx) = mpsc::channel(1);
        tx.send(PublicStreamMessage::Terminal(Err(error::websocket_error(
            PUBLIC_STREAM_EVENT_OPERATION,
            "socket failed",
        ))))
        .await
        .expect("terminal SDK error should enqueue");
        drop(tx);

        let mut stream = BinancePublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            shutdown_task: None,
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        let err = stream
            .next()
            .await
            .expect_err("SDK error should terminate the stream");

        assert_eq!(err.kind(), ErrorKind::Transport);
        assert_eq!(err.operation(), Some(PUBLIC_STREAM_EVENT_OPERATION));
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[tokio::test]
    async fn event_stream_treats_sdk_close_as_end_of_stream() {
        let (tx, rx) = mpsc::channel(1);
        tx.send(PublicStreamMessage::Terminal(Ok(())))
            .await
            .expect("terminal SDK close should enqueue");
        drop(tx);

        let mut stream = BinancePublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            shutdown_task: None,
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        assert!(matches!(stream.next().await, Ok(None)));
        assert!(matches!(stream.next().await, Ok(None)));
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
