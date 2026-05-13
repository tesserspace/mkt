use std::fmt;

use secrecy::ExposeSecret;
pub use secrecy::SecretString;

use mkt_types::ExchangeId;

pub struct ApiCredentials {
    api_key: SecretString,
    secret: SecretString,
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
    pub fn new(api_key: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            api_key: SecretString::from(api_key.into()),
            secret: SecretString::from(secret.into()),
            passphrase: None,
        }
    }

    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(SecretString::from(passphrase.into()));
        self
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

#[derive(Debug, PartialEq, Eq)]
pub struct ExchangeConfig {
    pub exchange_id: ExchangeId,
    pub rest_base_url: Option<String>,
    pub websocket_base_url: Option<String>,
    pub credentials: Option<ApiCredentials>,
}

impl ExchangeConfig {
    pub fn new(exchange_id: ExchangeId) -> Self {
        Self {
            exchange_id,
            rest_base_url: None,
            websocket_base_url: None,
            credentials: None,
        }
    }

    pub fn with_rest_base_url(mut self, url: impl Into<String>) -> Self {
        self.rest_base_url = Some(url.into());
        self
    }

    pub fn with_websocket_base_url(mut self, url: impl Into<String>) -> Self {
        self.websocket_base_url = Some(url.into());
        self
    }

    pub fn with_credentials(mut self, credentials: ApiCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }
}
