use async_trait::async_trait;
use reqwest::{header, Client, StatusCode};
use std::error::Error;
use std::time::{Duration, Instant};
use storm_core::{ByteRange, DataSink, Downloader, HttpVersion, ResourceInfo, StormError};
use url::Url;

pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    pub fn new() -> Result<Self, StormError> {
        let client = Client::builder()
            .user_agent("StormDL/0.1")
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(30))
            .http2_adaptive_window(true)
            .http2_initial_stream_window_size(2 * 1024 * 1024)
            .http2_initial_connection_window_size(4 * 1024 * 1024)
            .build()
            .map_err(|e| StormError::Network(e.to_string()))?;

        Ok(Self { client })
    }

    pub fn turbo() -> Result<Self, StormError> {
        let client = Client::builder()
            .user_agent("StormDL/0.1")
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(120))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(30))
            .http2_adaptive_window(true)
            .http2_initial_stream_window_size(4 * 1024 * 1024)
            .http2_initial_connection_window_size(8 * 1024 * 1024)
            .build()
            .map_err(|e| StormError::Network(e.to_string()))?;

        Ok(Self { client })
    }

    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    fn parse_content_disposition(header: &str) -> Option<String> {
        header
            .split(';')
            .find_map(|part| {
                let part = part.trim();
                if part.starts_with("filename=") {
                    Some(
                        part.trim_start_matches("filename=")
                            .trim_matches('"')
                            .to_string(),
                    )
                } else if part.starts_with("filename*=") {
                    let encoded = part.trim_start_matches("filename*=");
                    if let Some(name) = encoded.split("''").nth(1) {
                        urlencoding::decode(name).ok().map(|s| s.into_owned())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }
}

impl Default for HttpDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}

#[async_trait]
impl Downloader for HttpDownloader {
    async fn probe(&self, url: &Url) -> Result<ResourceInfo, StormError> {
        let start_time = Instant::now();
        let response = self
            .client
            .get(url.clone())
            .header(header::RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    StormError::Network(format!("Connection failed: {}", e))
                } else if e.is_timeout() {
                    StormError::Timeout(e.to_string())
                } else {
                    StormError::Network(format!("{}: {:?}", e, e.source()))
                }
            })?;
        let connection_rtt = start_time.elapsed();

        if !response.status().is_success() {
            return Err(StormError::Http {
                status: response.status().as_u16(),
                message: response.status().to_string(),
            });
        }

        let headers = response.headers();
        let status = response.status();

        let (size, supports_range) = if status == StatusCode::PARTIAL_CONTENT {
            let size = headers
                .get(header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').last())
                .and_then(|s| s.parse().ok());
            (size, true)
        } else {
            let size = headers
                .get(header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            let supports_range = headers
                .get(header::ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v == "bytes")
                .unwrap_or(false);
            (size, supports_range)
        };

        let etag = headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let last_modified = headers
            .get(header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let filename = headers
            .get(header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(Self::parse_content_disposition)
            .or_else(|| {
                url.path_segments()
                    .and_then(|segments| segments.last())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            });

        let http_version = match response.version() {
            reqwest::Version::HTTP_2 => HttpVersion::Http2,
            reqwest::Version::HTTP_3 => HttpVersion::Http3,
            _ => HttpVersion::Http1_1,
        };

        Ok(ResourceInfo {
            url: url.clone(),
            size,
            supports_range,
            etag,
            last_modified,
            content_type,
            filename,
            http_version,
            connection_rtt: Some(connection_rtt),
        })
    }

    async fn fetch_range(
        &self,
        url: &Url,
        range: ByteRange,
        sink: &mut dyn DataSink,
    ) -> Result<(), StormError> {
        use futures_util::StreamExt;

        let range_header = format!("bytes={}-{}", range.start, range.end - 1);

        let response = self
            .client
            .get(url.clone())
            .header(header::RANGE, range_header)
            .send()
            .await
            .map_err(|e| StormError::Network(e.to_string()))?;

        match response.status() {
            StatusCode::PARTIAL_CONTENT => {}
            StatusCode::OK => {
                return Err(StormError::RangeNotSupported);
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(StormError::RateLimited);
            }
            status => {
                return Err(StormError::Http {
                    status: status.as_u16(),
                    message: status.to_string(),
                });
            }
        }

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| StormError::Network(e.to_string()))?;
            sink.write(chunk)?;
        }
        sink.flush()?;

        Ok(())
    }

    async fn fetch_full(&self, url: &Url, sink: &mut dyn DataSink) -> Result<(), StormError> {
        use futures_util::StreamExt;

        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| StormError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(StormError::Http {
                status: response.status().as_u16(),
                message: response.status().to_string(),
            });
        }

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| StormError::Network(e.to_string()))?;
            sink.write(chunk)?;
        }
        sink.flush()?;

        Ok(())
    }
}
