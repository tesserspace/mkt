//! Core abstractions for exchange adapters and strategy-facing code.

mod capabilities;
mod config;
pub mod error;
mod handle;
mod stream;
mod traits;

pub use capabilities::{
    Capabilities, Capability, RestCapabilities, StreamCapabilities, TransportControl,
};
/// Re-exported secret string type used by API credentials.
pub use config::{ApiCredentials, ExchangeConfig, SecretString};
pub use error::{CapabilityUnavailableReason, Error, ErrorKind, Result};
pub use handle::{Builder, ExchangeHandle};
pub use mkt_types::BookDepthUpdateSpeed;
pub use secrecy::ExposeSecret;
pub use stream::{
    EventStream, MarketDataEvent, PrivateEvent, PrivateEventStream, PrivateSubscription,
    RawPayload, Subscription,
};
pub use traits::{
    Account, ExchangeInfo, FuturesTrading, MarketData, PrivateStream, PublicStream, SpotTrading,
};
