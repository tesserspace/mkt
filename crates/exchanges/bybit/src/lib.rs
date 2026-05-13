use std::sync::Arc;

use mkt_core::{Capabilities, ExchangeConfig, ExchangeInfo};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};
use reqwest::Client;
use url::Url;

#[non_exhaustive]
pub struct BybitClient {
    config: ExchangeConfig,
    http: Client,
    websocket_url: Option<Url>,
}

impl BybitClient {
    pub fn new(config: ExchangeConfig, http: Client, websocket_url: Option<Url>) -> Self {
        Self {
            config,
            http,
            websocket_url,
        }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn websocket_url(&self) -> Option<&Url> {
        self.websocket_url.as_ref()
    }
}

impl ExchangeInfo for BybitClient {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Bybit)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id()).with_markets([
            MarketKind::Spot,
            MarketKind::linear_perpetual(),
            MarketKind::inverse_perpetual(),
        ])
    }
}

impl From<BybitClient> for mkt_core::ExchangeHandle {
    fn from(value: BybitClient) -> Self {
        let client = Arc::new(value);
        let info: Arc<dyn ExchangeInfo> = client;
        Self::builder(info).build()
    }
}

impl std::fmt::Debug for BybitClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BybitClient")
            .field("config", &self.config)
            .field("has_http", &true)
            .field("websocket_url", &self.websocket_url)
            .finish()
    }
}
