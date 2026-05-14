use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use mkt_core::{ApiCredentials, EventStream, ExchangeConfig, MarketDataEvent, Subscription};
use mkt_exchange_binance::BinanceClient;
use mkt_types::{KnownExchange, OrderBook, Symbol};
use tokio::time::timeout;

const BINANCE_SPOT_TESTNET_REST_URL: &str = "https://testnet.binance.vision";
const BINANCE_SPOT_TESTNET_WS_STREAMS_URL: &str = "wss://stream.testnet.binance.vision";
const BINANCE_TESTNET_API_KEY: &str = "BINANCE_TESTNET_API_KEY";
const BINANCE_TESTNET_SECRET_KEY: &str = "BINANCE_TESTNET_SECRET_KEY";
const SMOKE_SYMBOL: &str = "BTCUSDT";
const REST_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_EVENT_TIMEOUT: Duration = Duration::from_secs(20);
const STREAM_CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn binance_testnet_account_balances_smoke() {
    let Some(handle) = testnet_handle() else {
        skip_missing_credentials();
        return;
    };

    timeout(
        REST_REQUEST_TIMEOUT,
        handle
            .account()
            .expect("Binance handle should bind account capability")
            .balances(),
    )
    .await
    .unwrap_or_else(|_| {
        panic!("Binance testnet account balances request should finish within timeout")
    })
    .unwrap_or_else(|_| panic!("Binance testnet account balances request should succeed"));
}

#[tokio::test]
async fn binance_testnet_public_order_book_stream_smoke() {
    let Some(handle) = testnet_handle() else {
        skip_missing_credentials();
        return;
    };

    let symbol = Symbol::spot(SMOKE_SYMBOL);
    let mut stream = timeout(
        STREAM_CONNECT_TIMEOUT,
        handle
            .public_stream()
            .expect("Binance handle should bind public stream capability")
            .subscribe_public(vec![Subscription::OrderBook {
                symbol: symbol.clone(),
                depth: Some(5),
            }]),
    )
    .await
    .unwrap_or_else(|_| {
        panic!("Binance testnet public stream subscription should finish within timeout")
    })
    .unwrap_or_else(|_| panic!("Binance testnet public stream subscription should succeed"));

    let book = timeout(
        STREAM_EVENT_TIMEOUT,
        next_order_book(stream.as_mut(), &symbol),
    )
    .await;

    timeout(STREAM_CLOSE_TIMEOUT, stream.close())
        .await
        .unwrap_or_else(|_| panic!("Binance testnet public stream should close within timeout"))
        .unwrap_or_else(|_| panic!("Binance testnet public stream should close cleanly"));

    let book = book
        .unwrap_or_else(|_| {
            panic!("Binance testnet public stream should emit order book event within timeout")
        })
        .unwrap_or_else(|message| panic!("{message}"));

    assert!(!book.bids.is_empty(), "order book should include bids");
    assert!(!book.asks.is_empty(), "order book should include asks");
}

async fn next_order_book(
    stream: &mut dyn EventStream,
    symbol: &Symbol,
) -> Result<OrderBook, &'static str> {
    loop {
        match stream.next().await {
            Ok(Some(MarketDataEvent::OrderBook(book))) if book.symbol == *symbol => {
                return Ok(book);
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err("Binance testnet public stream closed before order book event");
            }
            Err(_) => {
                return Err("Binance testnet public stream event should decode");
            }
        }
    }
}

fn testnet_handle() -> Option<mkt_core::ExchangeHandle> {
    let credentials = testnet_credentials()?;
    let config = ExchangeConfig::builder()
        .exchange_id(KnownExchange::Binance)
        .rest_base_url(BINANCE_SPOT_TESTNET_REST_URL)
        .websocket_base_url(BINANCE_SPOT_TESTNET_WS_STREAMS_URL)
        .credentials(credentials)
        .build()
        .expect("testnet exchange config should build");

    Some(
        BinanceClient::new(config)
            .expect("Binance testnet client should build")
            .into(),
    )
}

fn testnet_credentials() -> Option<ApiCredentials> {
    let api_key = secret_value(BINANCE_TESTNET_API_KEY)?;
    let secret = secret_value(BINANCE_TESTNET_SECRET_KEY)?;

    Some(
        ApiCredentials::builder()
            .api_key(api_key)
            .secret(secret)
            .build()
            .expect("testnet API credentials should build"),
    )
}

fn secret_value(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| dotenv_value(key))
}

fn dotenv_value(key: &str) -> Option<String> {
    candidate_dotenv_paths()
        .into_iter()
        .find_map(|path| dotenv_value_from_path(path.as_path(), key))
}

fn candidate_dotenv_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        paths.push(current_dir.join(".env"));
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    paths.push(manifest_dir.join(".env"));
    if let Some(workspace_dir) = manifest_dir.ancestors().nth(3) {
        paths.push(workspace_dir.join(".env"));
    }

    paths.sort();
    paths.dedup();
    paths
}

fn dotenv_value_from_path(path: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (name, value) = line.split_once('=')?;
        if name.trim() != key {
            return None;
        }

        Some(unquote_dotenv_value(value.trim()))
    })
}

fn unquote_dotenv_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_owned()
}

fn skip_missing_credentials() {
    eprintln!(
        "skipping Binance testnet smoke test: {BINANCE_TESTNET_API_KEY} and {BINANCE_TESTNET_SECRET_KEY} are not set"
    );
}
