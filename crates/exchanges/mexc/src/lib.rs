use std::sync::Arc;

use mkt_core::{
    Capabilities, Capability, ExchangeConfig, ExchangeInfo, RestCapabilities, StreamCapabilities,
    TransportControl,
};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};
use reqwest::Client;
use url::Url;

mod convert;
mod error;
pub mod ext;
mod market_data;
mod protobuf;
mod rest;
mod spot;
mod stream;

use market_data::MexcMarketData;
use rest::{base_rest_url, base_websocket_url};
use spot::{MexcAccount, MexcSpotTrading};
use stream::MexcPublicStream;

#[derive(Clone)]
#[non_exhaustive]
pub struct MexcClient {
    inner: Arc<MexcInner>,
}

pub(crate) struct MexcInner {
    pub(crate) config: ExchangeConfig,
    pub(crate) http: Client,
    pub(crate) rest_base_url: Url,
    pub(crate) websocket_base_url: Url,
}

impl MexcClient {
    pub fn new(config: ExchangeConfig) -> mkt_core::Result<Self> {
        Self::with_http(config, Client::new())
    }

    pub fn with_http(config: ExchangeConfig, http: Client) -> mkt_core::Result<Self> {
        let rest_base_url =
            base_rest_url(config.rest_base_url.as_deref()).map_err(|err| -> mkt_core::Error {
                mkt_core::Error::invalid_config(err)
                    .exchange(ExchangeId::from(KnownExchange::Mexc))
                    .config_key("config.rest_base_url")
                    .into()
            })?;
        let websocket_base_url = base_websocket_url(config.websocket_base_url.as_deref()).map_err(
            |err| -> mkt_core::Error {
                mkt_core::Error::invalid_config(err)
                    .exchange(ExchangeId::from(KnownExchange::Mexc))
                    .config_key("config.websocket_base_url")
                    .into()
            },
        )?;

        Ok(Self {
            inner: Arc::new(MexcInner {
                config,
                http,
                rest_base_url,
                websocket_base_url,
            }),
        })
    }

    pub fn http(&self) -> &Client {
        &self.inner.http
    }

    pub fn into_handle(self) -> mkt_core::ExchangeHandle {
        let inner = self.inner;

        mkt_core::ExchangeHandle::builder(Arc::new(MexcInfo))
            .market_data(Arc::new(MexcMarketData::new(Arc::clone(&inner))))
            .public_stream(Arc::new(MexcPublicStream::new(Arc::clone(&inner))))
            .spot_trading(Arc::new(MexcSpotTrading::new(Arc::clone(&inner))))
            .account(Arc::new(MexcAccount::new(inner)))
            .build()
    }
}

impl MexcInner {
    pub(crate) fn exchange_id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Mexc)
    }
}

impl From<MexcClient> for mkt_core::ExchangeHandle {
    fn from(value: MexcClient) -> Self {
        value.into_handle()
    }
}

struct MexcInfo;

impl ExchangeInfo for MexcInfo {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Mexc)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id())
            .with_markets([MarketKind::Spot])
            .with_rest(
                RestCapabilities::default()
                    .with_market_data()
                    .with_spot_trading()
                    .with_account(),
            )
            .with_stream(StreamCapabilities::default().with_public())
            .with_transport(TransportControl::DirectManaged)
            .with_capabilities([
                Capability::MarketData,
                Capability::SpotTrading,
                Capability::Account,
                Capability::PublicStream,
            ])
    }
}

impl std::fmt::Debug for MexcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MexcClient")
            .field("config", &self.inner.config)
            .field("has_http", &true)
            .field("rest_base_url", &self.inner.rest_base_url)
            .field("websocket_base_url", &self.inner.websocket_base_url)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use mkt_core::{
        Capability, CapabilityUnavailableReason, Error, ExchangeConfig, TransportControl,
    };
    use reqwest::Client;

    use super::MexcClient;
    use mkt_types::{KnownExchange, MarketKind};

    #[test]
    fn spot_market_data_public_stream_trading_and_account_are_bound() {
        let handle: mkt_core::ExchangeHandle = MexcClient::with_http(
            ExchangeConfig::builder()
                .exchange_id(KnownExchange::Mexc)
                .build()
                .expect("MEXC config should build"),
            Client::new(),
        )
        .expect("default MEXC config should build a client")
        .into();

        assert!(handle.market_data().is_ok());
        assert!(handle.public_stream().is_ok());
        assert!(handle.spot_trading().is_ok());
        assert!(handle.account().is_ok());
        assert!(matches!(
            handle.private_stream(),
            Err(Error::CapabilityUnavailable {
                capability: Capability::PrivateStream,
                reason: CapabilityUnavailableReason::NotAdvertised,
                ..
            })
        ));
        assert!(matches!(
            handle.futures_trading(),
            Err(Error::CapabilityUnavailable {
                capability: Capability::FuturesTrading,
                reason: CapabilityUnavailableReason::NotAdvertised,
                ..
            })
        ));
    }

    #[test]
    fn capabilities_report_direct_transport_and_spot_only_market() {
        let handle: mkt_core::ExchangeHandle = MexcClient::with_http(
            ExchangeConfig::builder()
                .exchange_id(KnownExchange::Mexc)
                .build()
                .expect("MEXC config should build"),
            Client::new(),
        )
        .expect("default MEXC config should build a client")
        .into();
        let capabilities = handle.info().capabilities();

        assert_eq!(capabilities.transport, TransportControl::DirectManaged);
        assert_eq!(capabilities.markets, vec![MarketKind::Spot]);
        assert!(capabilities.stream.public);
    }

    #[test]
    fn default_constructor_matches_binance_style_config_only_api() {
        let client = MexcClient::new(
            ExchangeConfig::builder()
                .exchange_id(KnownExchange::Mexc)
                .build()
                .expect("MEXC config should build"),
        )
        .expect("default MEXC config should build a client");

        assert_eq!(client.inner.exchange_id(), KnownExchange::Mexc.into());
    }
}
