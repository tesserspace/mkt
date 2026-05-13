use mkt_types::{ExchangeId, MarketKind};
use strum_macros::{Display, EnumString, IntoStaticStr};

#[non_exhaustive]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Display, EnumString, IntoStaticStr,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum Capability {
    MarketData,
    Account,
    SpotTrading,
    FuturesTrading,
    PublicStream,
    PrivateStream,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RestCapabilities {
    pub market_data: bool,
    pub account: bool,
    pub spot_trading: bool,
    pub futures_trading: bool,
}

impl RestCapabilities {
    pub fn with_market_data(mut self) -> Self {
        self.market_data = true;
        self
    }

    pub fn with_account(mut self) -> Self {
        self.account = true;
        self
    }

    pub fn with_spot_trading(mut self) -> Self {
        self.spot_trading = true;
        self
    }

    pub fn with_futures_trading(mut self) -> Self {
        self.futures_trading = true;
        self
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamCapabilities {
    pub public: bool,
    pub private: bool,
}

impl StreamCapabilities {
    pub fn with_public(mut self) -> Self {
        self.public = true;
        self
    }

    pub fn with_private(mut self) -> Self {
        self.private = true;
        self
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportControl {
    /// The adapter delegates HTTP/WS lifecycle to an official exchange SDK.
    ///
    /// This is intentional for exchanges such as Binance where the official
    /// Rust SDK owns signing, generated DTOs, endpoint coverage, and its own
    /// reqwest/WebSocket stack. Shared mkt controls can still be adapted
    /// through SDK hooks where available, but callers must not assume a shared
    /// transport layer intercepts every request.
    OfficialSdkManaged { sdk: &'static str },
    /// The adapter owns its HTTP/WebSocket clients directly.
    DirectManaged,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub exchange_id: ExchangeId,
    pub markets: Vec<MarketKind>,
    pub rest: RestCapabilities,
    pub stream: StreamCapabilities,
    pub transport: TransportControl,
}

impl Capabilities {
    pub fn new(exchange_id: ExchangeId) -> Self {
        Self {
            exchange_id,
            markets: Vec::new(),
            rest: RestCapabilities::default(),
            stream: StreamCapabilities::default(),
            transport: TransportControl::DirectManaged,
        }
    }

    pub fn with_markets(mut self, markets: impl IntoIterator<Item = MarketKind>) -> Self {
        self.markets = markets.into_iter().collect();
        self.markets.sort();
        self.markets.dedup();
        self
    }

    pub fn supports_market(&self, market: MarketKind) -> bool {
        self.markets.contains(&market)
    }

    pub fn with_rest(mut self, rest: RestCapabilities) -> Self {
        self.rest = rest;
        self
    }

    pub fn with_stream(mut self, stream: StreamCapabilities) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_capabilities(mut self, capabilities: impl IntoIterator<Item = Capability>) -> Self {
        for capability in capabilities {
            match capability {
                Capability::MarketData => self.rest.market_data = true,
                Capability::Account => self.rest.account = true,
                Capability::SpotTrading => self.rest.spot_trading = true,
                Capability::FuturesTrading => self.rest.futures_trading = true,
                Capability::PublicStream => self.stream.public = true,
                Capability::PrivateStream => self.stream.private = true,
            }
        }

        self
    }

    pub fn supports_capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::MarketData => self.rest.market_data,
            Capability::Account => self.rest.account,
            Capability::SpotTrading => self.rest.spot_trading,
            Capability::FuturesTrading => self.rest.futures_trading,
            Capability::PublicStream => self.stream.public,
            Capability::PrivateStream => self.stream.private,
        }
    }

    pub fn with_transport(mut self, transport: TransportControl) -> Self {
        self.transport = transport;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Capabilities, Capability};
    use mkt_types::{ExchangeId, KnownExchange, MarketKind};

    #[test]
    fn market_capabilities_are_sorted_and_deduplicated() {
        let capabilities = Capabilities::new(ExchangeId::from(KnownExchange::Binance))
            .with_markets([
                MarketKind::Spot,
                MarketKind::linear_perpetual(),
                MarketKind::Spot,
            ]);

        assert_eq!(
            capabilities.markets,
            vec![MarketKind::Spot, MarketKind::linear_perpetual()]
        );
        assert!(capabilities.supports_market(MarketKind::Spot));
    }

    #[test]
    fn capability_queries_cover_rest_and_stream_flags() {
        let capabilities = Capabilities::new(ExchangeId::from(KnownExchange::Binance))
            .with_capabilities([Capability::MarketData, Capability::PrivateStream]);

        assert!(capabilities.supports_capability(Capability::MarketData));
        assert!(capabilities.supports_capability(Capability::PrivateStream));
        assert!(!capabilities.supports_capability(Capability::Account));
    }
}
