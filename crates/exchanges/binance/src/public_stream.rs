use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;
use binance_sdk::models::{WebsocketEvent, WebsocketStreamsConnectConfig};
use mkt_core::{EventStream, MarketDataEvent, PublicStream, Result, Subscription};
use tokio::{
    sync::{
        mpsc::{self, error::TrySendError},
        oneshot,
    },
    task::JoinHandle,
};

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
        let plan = convert::build_public_stream_plan(
            subscriptions.as_slice(),
            SUBSCRIBE_PUBLIC_OPERATION,
        )?;
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
                        match convert::market_data_events_from_ws_text(
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

#[cfg(test)]
mod tests {
    use mkt_core::{ErrorKind, EventStream};

    use super::*;

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
}
