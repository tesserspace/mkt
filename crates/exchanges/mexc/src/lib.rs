use std::sync::Arc;

use mkt_core::{Capabilities, ExchangeConfig, ExchangeInfo};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};
use reqwest::Client;

#[non_exhaustive]
pub struct MexcClient {
    config: ExchangeConfig,
    http: Client,
}

impl MexcClient {
    pub fn new(config: ExchangeConfig, http: Client) -> Self {
        Self { config, http }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }
}

impl ExchangeInfo for MexcClient {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Mexc)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id())
            .with_markets([MarketKind::Spot, MarketKind::linear_perpetual()])
    }
}

impl From<MexcClient> for mkt_core::ExchangeHandle {
    fn from(value: MexcClient) -> Self {
        let client = Arc::new(value);
        let info: Arc<dyn ExchangeInfo> = client;
        Self::builder(info).build()
    }
}

impl std::fmt::Debug for MexcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MexcClient")
            .field("config", &self.config)
            .field("has_http", &true)
            .finish()
    }
}
