use std::sync::Arc;

use binance_sdk::{
    config::{ConfigurationRestApi, ConfigurationWebsocketStreams},
    derivatives_trading_usds_futures::{
        rest_api::RestApi as UsdsFuturesRestApi,
        websocket_streams::WebsocketStreamsHandle as UsdsFuturesWsStreamsHandle,
        DerivativesTradingUsdsFuturesRestApi, DerivativesTradingUsdsFuturesWsStreams,
    },
    spot::{
        rest_api::RestApi as SpotRestApiClient,
        websocket_streams::WebsocketStreamsHandle as SpotWsStreamsHandle, SpotRestApi,
        SpotWsStreams,
    },
};
use mkt_core::{
    Capabilities, Capability, ExchangeConfig, ExchangeInfo, ExposeSecret, RestCapabilities,
    StreamCapabilities, TransportControl,
};
use mkt_types::{ExchangeId, KnownExchange, MarketKind};

mod convert;
mod error;
pub mod ext;
mod market_data;
mod spot;

use market_data::BinanceMarketData;
use spot::{BinanceAccount, BinanceSpotTrading};

#[derive(Clone)]
pub struct BinanceClient {
    inner: Arc<BinanceInner>,
}

pub(crate) struct BinanceInner {
    pub(crate) config: ExchangeConfig,
    pub(crate) spot_rest: SpotRestApiClient,
    pub(crate) usds_futures_rest: UsdsFuturesRestApi,
    pub(crate) spot_ws_streams: SpotWsStreamsHandle,
    pub(crate) usds_futures_ws_streams: UsdsFuturesWsStreamsHandle,
}

impl BinanceClient {
    pub fn new(config: ExchangeConfig) -> mkt_core::Result<Self> {
        let mut rest_builder = ConfigurationRestApi::builder();

        if let Some(credentials) = &config.credentials {
            rest_builder = rest_builder
                .api_key(credentials.api_key().expose_secret().to_owned())
                .api_secret(credentials.secret().expose_secret().to_owned());
        }

        if let Some(base_path) = &config.rest_base_url {
            rest_builder = rest_builder.base_path(base_path.clone());
        }

        let rest_config = rest_builder.build().map_err(|err| -> mkt_core::Error {
            mkt_core::Error::invalid_config(err.to_string())
                .exchange(ExchangeId::from(KnownExchange::Binance))
                .config_key("config.rest")
                .into()
        })?;

        let mut ws_streams_builder = ConfigurationWebsocketStreams::builder();

        if let Some(ws_url) = &config.websocket_base_url {
            ws_streams_builder = ws_streams_builder.ws_url(ws_url.clone());
        }

        let ws_streams_config = ws_streams_builder
            .build()
            .map_err(|err| -> mkt_core::Error {
                mkt_core::Error::invalid_config(err.to_string())
                    .exchange(ExchangeId::from(KnownExchange::Binance))
                    .config_key("config.ws_streams")
                    .into()
            })?;

        let spot_rest = SpotRestApi::from_config(rest_config.clone());
        let usds_futures_rest = DerivativesTradingUsdsFuturesRestApi::from_config(rest_config);
        let spot_ws_streams = SpotWsStreams::from_config(ws_streams_config.clone());
        let usds_futures_ws_streams =
            DerivativesTradingUsdsFuturesWsStreams::from_config(ws_streams_config);

        Ok(Self {
            inner: Arc::new(BinanceInner {
                config,
                spot_rest,
                usds_futures_rest,
                spot_ws_streams,
                usds_futures_ws_streams,
            }),
        })
    }

    pub fn raw(&self) -> BinanceRawApi<'_> {
        BinanceRawApi { client: self }
    }

    pub fn into_handle(self) -> mkt_core::ExchangeHandle {
        let inner = self.inner;

        mkt_core::ExchangeHandle::builder(Arc::new(BinanceInfo))
            .market_data(Arc::new(BinanceMarketData::new(inner.clone())))
            .spot_trading(Arc::new(BinanceSpotTrading::new(inner.clone())))
            .account(Arc::new(BinanceAccount::new(inner)))
            .build()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BinanceRawApi<'a> {
    client: &'a BinanceClient,
}

impl<'a> BinanceRawApi<'a> {
    pub fn spot_rest(self) -> &'a SpotRestApiClient {
        &self.client.inner.spot_rest
    }

    pub fn usds_futures_rest(self) -> &'a UsdsFuturesRestApi {
        &self.client.inner.usds_futures_rest
    }

    pub fn spot_ws_streams(self) -> &'a SpotWsStreamsHandle {
        &self.client.inner.spot_ws_streams
    }

    pub fn usds_futures_ws_streams(self) -> &'a UsdsFuturesWsStreamsHandle {
        &self.client.inner.usds_futures_ws_streams
    }
}

impl From<BinanceClient> for mkt_core::ExchangeHandle {
    fn from(value: BinanceClient) -> Self {
        value.into_handle()
    }
}

struct BinanceInfo;

impl ExchangeInfo for BinanceInfo {
    fn id(&self) -> ExchangeId {
        ExchangeId::from(KnownExchange::Binance)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(self.id())
            .with_markets([
                MarketKind::Spot,
                MarketKind::linear_perpetual(),
                MarketKind::inverse_perpetual(),
            ])
            .with_rest(
                RestCapabilities::default()
                    .with_market_data()
                    .with_spot_trading()
                    .with_account(),
            )
            .with_stream(StreamCapabilities::default())
            .with_transport(TransportControl::OfficialSdkManaged { sdk: "binance-sdk" })
            .with_capabilities([
                Capability::MarketData,
                Capability::SpotTrading,
                Capability::Account,
            ])
    }
}

impl std::fmt::Debug for BinanceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinanceClient")
            .field("config", &self.inner.config)
            .field("sdk", &"binance-sdk")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use mkt_core::{
        Capability, CapabilityUnavailableReason, Error, ExchangeConfig, TransportControl,
    };
    use mkt_types::{ExchangeId, KnownExchange};

    use super::BinanceClient;

    #[test]
    fn capabilities_report_official_sdk_transport_control() {
        let handle: mkt_core::ExchangeHandle = BinanceClient::new(ExchangeConfig::new(
            ExchangeId::from(KnownExchange::Binance),
        ))
        .expect("official SDK configuration should build")
        .into();
        let capabilities = handle.info().capabilities();

        assert_eq!(
            capabilities.transport,
            TransportControl::OfficialSdkManaged { sdk: "binance-sdk" }
        );
    }

    #[test]
    fn market_data_spot_trading_and_account_are_bound_into_handle() {
        let handle: mkt_core::ExchangeHandle = BinanceClient::new(ExchangeConfig::new(
            ExchangeId::from(KnownExchange::Binance),
        ))
        .expect("official SDK configuration should build")
        .into();

        assert!(handle.market_data().is_ok());
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
    }
}
