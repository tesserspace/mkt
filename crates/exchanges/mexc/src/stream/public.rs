use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use mkt_core::{EventStream, MarketDataEvent, PublicStream, Result, Subscription};
use serde::Serialize;
use tokio::{
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame, Message},
        Utf8Bytes,
    },
};

use super::{
    message::{market_data_events_from_ws_frame, MexcWsFrame},
    plan::{MexcChannel, MexcPublicStreamPlan, MexcPublicStreamRoute},
};
use crate::{error, MexcInner};

const SUBSCRIBE_PUBLIC_OPERATION: &str = "spot.public_stream.subscribe";
const PUBLIC_STREAM_EVENT_OPERATION: &str = "spot.public_stream.event";
const EVENT_BUFFER_CAPACITY: usize = 1024;
const SUBSCRIPTION_METHOD: &str = "SUBSCRIPTION";
const UNSUBSCRIPTION_METHOD: &str = "UNSUBSCRIPTION";
const PING_METHOD: &str = "PING";
const PING_INTERVAL: Duration = Duration::from_secs(20);

pub(crate) struct MexcPublicStream {
    inner: Arc<MexcInner>,
}

impl MexcPublicStream {
    pub(crate) fn new(inner: Arc<MexcInner>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl PublicStream for MexcPublicStream {
    async fn subscribe_public(
        &self,
        subscriptions: Vec<Subscription>,
    ) -> Result<Box<dyn EventStream>> {
        let plan =
            MexcPublicStreamPlan::build(subscriptions.as_slice(), SUBSCRIBE_PUBLIC_OPERATION)?;
        let channels: Vec<String> = plan.channels.iter().cloned().map(String::from).collect();
        let routes = Arc::new(plan.routes);
        let (websocket, _) = connect_async(self.inner.websocket_base_url.as_str())
            .await
            .map_err(|err| websocket_error(SUBSCRIBE_PUBLIC_OPERATION, err.to_string()))?;
        let (mut writer, mut reader) = websocket.split();

        writer
            .send(Message::text(
                serde_json::to_string(&MexcControlRequest::new(
                    SUBSCRIPTION_METHOD,
                    channels.clone(),
                ))
                .map_err(|err| error::decode_error(SUBSCRIBE_PUBLIC_OPERATION, err.to_string()))?,
            ))
            .await
            .map_err(|err| websocket_error(SUBSCRIBE_PUBLIC_OPERATION, err.to_string()))?;

        let (tx, rx) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        let overflowed = Arc::new(AtomicBool::new(false));
        let terminal_sent = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let overflowed_task = Arc::clone(&overflowed);
        let terminal_sent_task = Arc::clone(&terminal_sent);

        let event_task = tokio::spawn(async move {
            let mut ping_interval = time::interval(PING_INTERVAL);
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        let _ = writer
                            .send(Message::text(
                                serde_json::to_string(&MexcControlRequest::new(UNSUBSCRIPTION_METHOD, channels.clone()))
                                    .unwrap_or_else(|_| "{\"method\":\"UNSUBSCRIPTION\",\"params\":[]}".to_owned()),
                            ))
                            .await;
                        let _ = writer
                            .send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Normal,
                                reason: Utf8Bytes::from_static("client shutdown"),
                            })))
                            .await;
                        break;
                    }
                    _ = ping_interval.tick() => {
                        if terminal_sent_task.load(Ordering::Acquire) || overflowed_task.load(Ordering::Acquire) {
                            break;
                        }
                        if let Err(err) = writer.send(Message::text(ping_request())).await {
                            enqueue_terminal(
                                &tx,
                                &overflowed_task,
                                &terminal_sent_task,
                                Err(websocket_error(PUBLIC_STREAM_EVENT_OPERATION, err.to_string())),
                            );
                            break;
                        }
                    }
                    incoming = reader.next() => {
                        if terminal_sent_task.load(Ordering::Acquire) || overflowed_task.load(Ordering::Acquire) {
                            break;
                        }

                        match incoming {
                            Some(Ok(message)) => {
                                enqueue_websocket_message(
                                    &tx,
                                    &overflowed_task,
                                    &terminal_sent_task,
                                    message,
                                    routes.as_ref(),
                                );
                            }
                            Some(Err(err)) => {
                                enqueue_terminal(
                                    &tx,
                                    &overflowed_task,
                                    &terminal_sent_task,
                                    Err(websocket_error(PUBLIC_STREAM_EVENT_OPERATION, err.to_string())),
                                );
                                break;
                            }
                            None => {
                                enqueue_terminal(&tx, &overflowed_task, &terminal_sent_task, Ok(()));
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Box::new(MexcPublicEventStream {
            receiver: rx,
            shutdown_tx: Some(shutdown_tx),
            event_task: Some(event_task),
            overflowed,
            terminated: false,
        }))
    }
}

enum PublicStreamMessage {
    Event(Result<MarketDataEvent>),
    Terminal(Result<()>),
}

pub(super) struct MexcPublicEventStream {
    receiver: mpsc::Receiver<PublicStreamMessage>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    event_task: Option<JoinHandle<()>>,
    overflowed: Arc<AtomicBool>,
    terminated: bool,
}

impl MexcPublicEventStream {
    async fn shutdown(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(event_task) = self.event_task.take() {
            let _ = event_task.await;
        }
    }

    async fn terminate_with(&mut self, result: Result<()>) -> Result<Option<MarketDataEvent>> {
        self.terminated = true;
        self.shutdown().await;
        result.map(|()| None)
    }
}

#[async_trait]
impl EventStream for MexcPublicEventStream {
    async fn next(&mut self) -> Result<Option<MarketDataEvent>> {
        if self.terminated {
            return Ok(None);
        }

        if self.overflowed.load(Ordering::Acquire) {
            return self
                .terminate_with(Err(websocket_error(
                    PUBLIC_STREAM_EVENT_OPERATION,
                    format!(
                        "MEXC public websocket event buffer overflowed after {EVENT_BUFFER_CAPACITY} queued events"
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

impl Drop for MexcPublicEventStream {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

#[derive(Debug, Serialize)]
struct MexcControlRequest {
    method: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    params: Vec<String>,
}

impl MexcControlRequest {
    fn new(method: &'static str, params: Vec<String>) -> Self {
        Self { method, params }
    }
}

fn enqueue_websocket_message(
    tx: &mpsc::Sender<PublicStreamMessage>,
    overflowed: &AtomicBool,
    terminal_sent: &AtomicBool,
    message: Message,
    routes: &BTreeMap<MexcChannel, MexcPublicStreamRoute>,
) {
    let frames = match message {
        Message::Text(raw) => Some(MexcWsFrame::Text(raw.to_string())),
        Message::Binary(raw) => Some(MexcWsFrame::Binary(raw.to_vec())),
        Message::Close(None) => {
            enqueue_terminal(tx, overflowed, terminal_sent, Ok(()));
            return;
        }
        Message::Close(Some(frame)) if frame.code == CloseCode::Normal => {
            enqueue_terminal(tx, overflowed, terminal_sent, Ok(()));
            return;
        }
        Message::Close(Some(frame)) => {
            enqueue_terminal(
                tx,
                overflowed,
                terminal_sent,
                Err(websocket_error(
                    PUBLIC_STREAM_EVENT_OPERATION,
                    format!(
                        "MEXC public websocket closed with code {}: {}",
                        u16::from(frame.code),
                        frame.reason
                    ),
                )),
            );
            return;
        }
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => None,
    };

    if let Some(frame) = frames {
        match market_data_events_from_ws_frame(frame, routes, PUBLIC_STREAM_EVENT_OPERATION) {
            Ok(events) => {
                for event in events {
                    enqueue_event(tx, overflowed, terminal_sent, Ok(event));
                }
            }
            Err(err) => enqueue_event(tx, overflowed, terminal_sent, Err(err)),
        }
    }
}

fn enqueue_event(
    tx: &mpsc::Sender<PublicStreamMessage>,
    overflowed: &AtomicBool,
    terminal_sent: &AtomicBool,
    event: Result<MarketDataEvent>,
) {
    enqueue_message(
        tx,
        overflowed,
        terminal_sent,
        PublicStreamMessage::Event(event),
    );
}

fn enqueue_terminal(
    tx: &mpsc::Sender<PublicStreamMessage>,
    overflowed: &AtomicBool,
    terminal_sent: &AtomicBool,
    terminal: Result<()>,
) {
    enqueue_message(
        tx,
        overflowed,
        terminal_sent,
        PublicStreamMessage::Terminal(terminal),
    );
}

fn enqueue_message(
    tx: &mpsc::Sender<PublicStreamMessage>,
    overflowed: &AtomicBool,
    terminal_sent: &AtomicBool,
    message: PublicStreamMessage,
) {
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

fn websocket_error(operation: &'static str, message: impl Into<String>) -> mkt_core::Error {
    mkt_core::Error::transport(message.into())
        .exchange(mkt_types::ExchangeId::from(mkt_types::KnownExchange::Mexc))
        .operation(operation)
        .into()
}

fn ping_request() -> String {
    serde_json::to_string(&MexcControlRequest::new(PING_METHOD, Vec::new()))
        .unwrap_or_else(|_| "{\"method\":\"PING\"}".to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use mkt_core::{ErrorKind, EventStream, MarketDataEvent};

    use super::*;

    #[tokio::test]
    async fn event_stream_reports_buffer_overflow_as_terminal_error() {
        let (_tx, rx) = mpsc::channel(1);
        let mut stream = MexcPublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            event_task: None,
            overflowed: Arc::new(AtomicBool::new(true)),
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
    async fn explicit_close_waits_for_event_task_shutdown() {
        let (_tx, rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let closed = Arc::new(AtomicBool::new(false));
        let closed_task = Arc::clone(&closed);
        let event_task = tokio::spawn(async move {
            shutdown_rx
                .await
                .expect("explicit close should notify shutdown task");
            closed_task.store(true, Ordering::Release);
        });
        let mut stream = MexcPublicEventStream {
            receiver: rx,
            shutdown_tx: Some(shutdown_tx),
            event_task: Some(event_task),
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        stream
            .close()
            .await
            .expect("explicit close should finish shutdown task");

        assert!(stream.shutdown_tx.is_none());
        assert!(stream.event_task.is_none());
        assert!(closed.load(Ordering::Acquire));
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[tokio::test]
    async fn event_stream_treats_terminal_error_as_end_after_reporting_it() {
        let (tx, rx) = mpsc::channel(1);
        tx.send(PublicStreamMessage::Terminal(Err(websocket_error(
            PUBLIC_STREAM_EVENT_OPERATION,
            "socket failed",
        ))))
        .await
        .expect("terminal error should enqueue");
        drop(tx);

        let mut stream = MexcPublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            event_task: None,
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        let err = stream
            .next()
            .await
            .expect_err("terminal error should be reported");

        assert_eq!(err.kind(), ErrorKind::Transport);
        assert_eq!(err.operation(), Some(PUBLIC_STREAM_EVENT_OPERATION));
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[tokio::test]
    async fn event_stream_returns_queued_event_before_clean_close() {
        let (tx, rx) = mpsc::channel(2);
        tx.send(PublicStreamMessage::Event(Ok(MarketDataEvent::Raw {
            exchange_id: mkt_types::ExchangeId::from(mkt_types::KnownExchange::Mexc),
            payload: mkt_core::RawPayload::Text("ok".to_owned()),
        })))
        .await
        .expect("event should enqueue");
        tx.send(PublicStreamMessage::Terminal(Ok(())))
            .await
            .expect("terminal close should enqueue");
        drop(tx);

        let mut stream = MexcPublicEventStream {
            receiver: rx,
            shutdown_tx: None,
            event_task: None,
            overflowed: Arc::new(AtomicBool::new(false)),
            terminated: false,
        };

        assert!(matches!(
            stream.next().await,
            Ok(Some(MarketDataEvent::Raw { .. }))
        ));
        assert!(matches!(stream.next().await, Ok(None)));
        assert!(matches!(stream.next().await, Ok(None)));
    }

    #[test]
    fn enqueue_close_frame_as_terminal_close() {
        let (tx, mut rx) = mpsc::channel(1);
        enqueue_websocket_message(
            &tx,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            Message::Close(None),
            &std::collections::BTreeMap::new(),
        );

        assert!(matches!(
            rx.try_recv(),
            Ok(PublicStreamMessage::Terminal(Ok(())))
        ));
    }

    #[test]
    fn enqueue_text_control_frame_ignores_subscription_ack() {
        let (tx, mut rx) = mpsc::channel(1);
        enqueue_websocket_message(
            &tx,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            Message::text(r#"{"code":0,"msg":"spot@public.kline.v3.api.pb@BTCUSDT@Min1"}"#),
            &std::collections::BTreeMap::new(),
        );

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn enqueue_text_unknown_frame_preserves_raw_message() {
        let (tx, mut rx) = mpsc::channel(1);
        enqueue_websocket_message(
            &tx,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            Message::text(r#"{"stream":"unexpected"}"#),
            &std::collections::BTreeMap::new(),
        );

        assert!(matches!(
            rx.try_recv(),
            Ok(PublicStreamMessage::Event(Ok(MarketDataEvent::Raw { .. })))
        ));
    }

    #[test]
    fn ping_request_uses_minimal_mexc_payload() {
        assert_eq!(ping_request(), r#"{"method":"PING"}"#);
    }

    #[test]
    fn subscription_request_keeps_params_when_present() {
        let payload = serde_json::to_string(&MexcControlRequest::new(
            SUBSCRIPTION_METHOD,
            vec!["spot@public.kline.v3.api.pb@BTCUSDT@Min1".to_owned()],
        ))
        .expect("control request fixture should serialize");

        assert_eq!(
            payload,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api.pb@BTCUSDT@Min1"]}"#
        );
    }
}
