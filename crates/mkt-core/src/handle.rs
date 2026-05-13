use std::sync::Arc;

use crate::{
    Account, Capabilities, Capability, CapabilityUnavailableReason, Error, ExchangeInfo,
    FuturesTrading, MarketData, PrivateStream, PublicStream, Result, SpotTrading,
};

#[derive(Clone)]
pub struct ExchangeHandle {
    info: Arc<dyn ExchangeInfo>,
    market_data: Option<Arc<dyn MarketData>>,
    spot_trading: Option<Arc<dyn SpotTrading>>,
    futures_trading: Option<Arc<dyn FuturesTrading>>,
    account: Option<Arc<dyn Account>>,
    public_stream: Option<Arc<dyn PublicStream>>,
    private_stream: Option<Arc<dyn PrivateStream>>,
}

impl ExchangeHandle {
    pub fn builder(info: Arc<dyn ExchangeInfo>) -> Builder {
        Builder::new(info)
    }

    pub fn info(&self) -> &dyn ExchangeInfo {
        self.info.as_ref()
    }

    pub fn market_data(&self) -> Result<&dyn MarketData> {
        self.market_data
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::MarketData))
    }

    pub fn spot_trading(&self) -> Result<&dyn SpotTrading> {
        self.spot_trading
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::SpotTrading))
    }

    pub fn futures_trading(&self) -> Result<&dyn FuturesTrading> {
        self.futures_trading
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::FuturesTrading))
    }

    pub fn account(&self) -> Result<&dyn Account> {
        self.account
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::Account))
    }

    pub fn public_stream(&self) -> Result<&dyn PublicStream> {
        self.public_stream
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::PublicStream))
    }

    pub fn private_stream(&self) -> Result<&dyn PrivateStream> {
        self.private_stream
            .as_deref()
            .ok_or_else(|| self.capability_error(Capability::PrivateStream))
    }

    pub fn capabilities(&self) -> Capabilities {
        let advertised = self.info.capabilities();

        Capabilities::new(self.info.id())
            .with_markets(advertised.markets)
            .with_transport(advertised.transport)
            .with_capabilities(
                [
                    self.market_data.is_some().then_some(Capability::MarketData),
                    self.spot_trading
                        .is_some()
                        .then_some(Capability::SpotTrading),
                    self.futures_trading
                        .is_some()
                        .then_some(Capability::FuturesTrading),
                    self.account.is_some().then_some(Capability::Account),
                    self.public_stream
                        .is_some()
                        .then_some(Capability::PublicStream),
                    self.private_stream
                        .is_some()
                        .then_some(Capability::PrivateStream),
                ]
                .into_iter()
                .flatten(),
            )
    }

    fn capability_error(&self, capability: Capability) -> Error {
        let reason = if self.info.capabilities().supports_capability(capability) {
            CapabilityUnavailableReason::NotBound
        } else {
            CapabilityUnavailableReason::NotAdvertised
        };

        Error::capability_unavailable(self.info.id(), capability, reason)
    }
}

#[derive(Clone)]
pub struct Builder {
    info: Arc<dyn ExchangeInfo>,
    market_data: Option<Arc<dyn MarketData>>,
    spot_trading: Option<Arc<dyn SpotTrading>>,
    futures_trading: Option<Arc<dyn FuturesTrading>>,
    account: Option<Arc<dyn Account>>,
    public_stream: Option<Arc<dyn PublicStream>>,
    private_stream: Option<Arc<dyn PrivateStream>>,
}

impl Builder {
    pub fn new(info: Arc<dyn ExchangeInfo>) -> Self {
        Self {
            info,
            market_data: None,
            spot_trading: None,
            futures_trading: None,
            account: None,
            public_stream: None,
            private_stream: None,
        }
    }

    pub fn market_data(mut self, market_data: Arc<dyn MarketData>) -> Self {
        self.market_data = Some(market_data);
        self
    }

    pub fn spot_trading(mut self, spot_trading: Arc<dyn SpotTrading>) -> Self {
        self.spot_trading = Some(spot_trading);
        self
    }

    pub fn futures_trading(mut self, futures_trading: Arc<dyn FuturesTrading>) -> Self {
        self.futures_trading = Some(futures_trading);
        self
    }

    pub fn account(mut self, account: Arc<dyn Account>) -> Self {
        self.account = Some(account);
        self
    }

    pub fn public_stream(mut self, public_stream: Arc<dyn PublicStream>) -> Self {
        self.public_stream = Some(public_stream);
        self
    }

    pub fn private_stream(mut self, private_stream: Arc<dyn PrivateStream>) -> Self {
        self.private_stream = Some(private_stream);
        self
    }

    pub fn build(self) -> ExchangeHandle {
        ExchangeHandle {
            info: self.info,
            market_data: self.market_data,
            spot_trading: self.spot_trading,
            futures_trading: self.futures_trading,
            account: self.account,
            public_stream: self.public_stream,
            private_stream: self.private_stream,
        }
    }
}

impl std::fmt::Debug for ExchangeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeHandle")
            .field("exchange_id", &self.info.id())
            .field("has_market_data", &self.market_data.is_some())
            .field("has_spot_trading", &self.spot_trading.is_some())
            .field("has_futures_trading", &self.futures_trading.is_some())
            .field("has_account", &self.account.is_some())
            .field("has_public_stream", &self.public_stream.is_some())
            .field("has_private_stream", &self.private_stream.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mkt_types::{ExchangeId, KnownExchange};

    use super::ExchangeHandle;
    use crate::{Capabilities, Capability, CapabilityUnavailableReason, Error, ExchangeInfo};

    struct StubExchange {
        capabilities: Capabilities,
    }

    impl ExchangeInfo for StubExchange {
        fn id(&self) -> ExchangeId {
            self.capabilities.exchange_id.clone()
        }

        fn capabilities(&self) -> Capabilities {
            self.capabilities.clone()
        }
    }

    #[test]
    fn missing_unadvertised_capability_reports_not_advertised() {
        let exchange_id = ExchangeId::from(KnownExchange::Binance);
        let handle = ExchangeHandle::builder(Arc::new(StubExchange {
            capabilities: Capabilities::new(exchange_id.clone()),
        }))
        .build();

        let err = handle
            .market_data()
            .err()
            .expect("market data is not advertised");

        assert!(matches!(
            err,
            Error::CapabilityUnavailable {
                exchange,
                capability: Capability::MarketData,
                reason: CapabilityUnavailableReason::NotAdvertised,
            } if exchange == exchange_id
        ));
    }

    #[test]
    fn advertised_but_unbound_capability_reports_binding_gap() {
        let exchange_id = ExchangeId::from(KnownExchange::Binance);
        let handle = ExchangeHandle::builder(Arc::new(StubExchange {
            capabilities: Capabilities::new(exchange_id.clone())
                .with_capabilities([Capability::MarketData]),
        }))
        .build();

        let err = handle
            .market_data()
            .err()
            .expect("market data was advertised but not bound");

        assert!(matches!(
            err,
            Error::CapabilityUnavailable {
                exchange,
                capability: Capability::MarketData,
                reason: CapabilityUnavailableReason::NotBound,
            } if exchange == exchange_id
        ));
    }
}
