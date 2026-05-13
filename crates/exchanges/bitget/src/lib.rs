use std::sync::Arc;

use mkt_core::{Capabilities, ExchangeConfig, ExchangeInfo};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};
use reqwest::Client;

#[non_exhaustive]
pub struct BitgetClient {
    config: ExchangeConfig,
    http: Client,
}

impl BitgetClient {
    pub fn new(config: ExchangeConfig, http: Client) -> Self {
        Self { config, http }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }
}

impl ExchangeInfo for BitgetClient {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Bitget)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id()).with_markets([
            MarketKind::Spot,
            MarketKind::linear_perpetual(),
            MarketKind::inverse_perpetual(),
        ])
    }
}

impl From<BitgetClient> for mkt_core::ExchangeHandle {
    fn from(value: BitgetClient) -> Self {
        let client = Arc::new(value);
        let info: Arc<dyn ExchangeInfo> = client;
        Self::builder(info).build()
    }
}

impl std::fmt::Debug for BitgetClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitgetClient")
            .field("config", &self.config)
            .field("has_http", &true)
            .finish()
    }
}
