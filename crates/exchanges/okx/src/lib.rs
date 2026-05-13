use std::sync::Arc;

use mkt_core::{Capabilities, ExchangeConfig, ExchangeInfo};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};
use reqwest::Client;

#[non_exhaustive]
pub struct OkxClient {
    config: ExchangeConfig,
    http: Client,
}

impl OkxClient {
    pub fn new(config: ExchangeConfig, http: Client) -> Self {
        Self { config, http }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }
}

impl ExchangeInfo for OkxClient {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Okx)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id()).with_markets([
            MarketKind::Spot,
            MarketKind::linear_perpetual(),
            MarketKind::inverse_perpetual(),
            MarketKind::linear_expiring(),
            MarketKind::inverse_expiring(),
        ])
    }
}

impl From<OkxClient> for mkt_core::ExchangeHandle {
    fn from(value: OkxClient) -> Self {
        let client = Arc::new(value);
        let info: Arc<dyn ExchangeInfo> = client;
        Self::builder(info).build()
    }
}

impl std::fmt::Debug for OkxClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OkxClient")
            .field("config", &self.config)
            .field("has_http", &true)
            .finish()
    }
}
