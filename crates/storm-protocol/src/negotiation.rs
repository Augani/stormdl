use reqwest::Client;
use std::time::Duration;
use stormdl_core::StormError;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreferredProtocol {
    Http1,
    Http2,
    Http3,
    #[default]
    Auto,
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
            Ok(response) => response
                .headers()
                .get("alt-svc")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("h3"))
                .unwrap_or(false),
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
