use std::fmt;

use derive_builder::Builder;
use secrecy::ExposeSecret;
pub use secrecy::SecretString;

use mkt_types::ExchangeId;

#[derive(Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct ApiCredentials {
    api_key: SecretString,
    secret: SecretString,
    #[builder(default)]
    passphrase: Option<SecretString>,
}

impl PartialEq for ApiCredentials {
    fn eq(&self, other: &Self) -> bool {
        self.api_key.expose_secret() == other.api_key.expose_secret()
            && self.secret.expose_secret() == other.secret.expose_secret()
            && self.passphrase.as_ref().map(|value| value.expose_secret())
                == other.passphrase.as_ref().map(|value| value.expose_secret())
    }
}

impl Eq for ApiCredentials {}

impl ApiCredentials {
    pub fn builder() -> ApiCredentialsBuilder {
        ApiCredentialsBuilder::default()
    }

    pub fn api_key(&self) -> &SecretString {
        &self.api_key
    }

    pub fn secret(&self) -> &SecretString {
        &self.secret
    }

    pub fn passphrase(&self) -> Option<&SecretString> {
        self.passphrase.as_ref()
    }
}

impl fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiCredentials")
            .field("api_key", &self.api_key)
            .field("secret", &self.secret)
            .field("passphrase", &self.passphrase)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Builder)]
#[builder(pattern = "owned", setter(into, strip_option))]
pub struct ExchangeConfig {
    pub exchange_id: ExchangeId,
    #[builder(default)]
    pub rest_base_url: Option<String>,
    #[builder(default)]
    pub websocket_base_url: Option<String>,
    #[builder(default)]
    pub credentials: Option<ApiCredentials>,
}

impl ExchangeConfig {
    pub fn builder() -> ExchangeConfigBuilder {
        ExchangeConfigBuilder::default()
    }
}
