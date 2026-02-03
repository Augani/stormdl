use reqwest::Client;
use std::time::Duration;
use storm_core::StormError;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredProtocol {
    Http1,
    Http2,
    Http3,
    Auto,
}

impl Default for PreferredProtocol {
    fn default() -> Self {
        Self::Auto
    }
}

pub struct ProtocolNegotiator {
    client: Client,
}

impl ProtocolNegotiator {
    pub fn new() -> Result<Self, StormError> {
        let client = Client::builder()
            .user_agent("StormDL/0.1")
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| StormError::Network(e.to_string()))?;

        Ok(Self { client })
    }

    pub async fn detect_http3_support(&self, url: &Url) -> bool {
        let result = self.client.head(url.clone()).send().await;

        match result {
            Ok(response) => {
                if let Some(alt_svc) = response.headers().get("alt-svc") {
                    if let Ok(value) = alt_svc.to_str() {
                        return value.contains("h3");
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    pub async fn negotiate(&self, url: &Url, preferred: PreferredProtocol) -> PreferredProtocol {
        match preferred {
            PreferredProtocol::Http1 | PreferredProtocol::Http2 | PreferredProtocol::Http3 => {
                preferred
            }
            PreferredProtocol::Auto => {
                if self.detect_http3_support(url).await {
                    PreferredProtocol::Http3
                } else {
                    PreferredProtocol::Http2
                }
            }
        }
    }
}

impl Default for ProtocolNegotiator {
    fn default() -> Self {
        Self::new().expect("Failed to create protocol negotiator")
    }
}
