use async_trait::async_trait;
use bytes::Buf;
use quinn::{ClientConfig, Endpoint, TransportConfig};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use storm_core::{ByteRange, DataSink, Downloader, HttpVersion, ResourceInfo, StormError};
use url::Url;

pub struct Http3Downloader {
    endpoint: Endpoint,
}

impl Http3Downloader {
    pub fn new() -> Result<Self, StormError> {
        let tls_config = Self::create_tls_config()?;

        let mut transport = TransportConfig::default();
        transport.max_idle_timeout(Some(
            Duration::from_secs(30)
                .try_into()
                .map_err(|e| StormError::Protocol(format!("Invalid timeout: {:?}", e)))?,
        ));
        transport.initial_mtu(1200);
        transport.min_mtu(1200);
        transport.keep_alive_interval(Some(Duration::from_secs(15)));

        let mut client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
                .map_err(|e| StormError::Protocol(format!("QUIC config error: {:?}", e)))?,
        ));
        client_config.transport_config(Arc::new(transport));

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| StormError::Network(format!("Failed to create endpoint: {}", e)))?;
        endpoint.set_default_client_config(client_config);

        Ok(Self { endpoint })
    }

    pub fn turbo() -> Result<Self, StormError> {
        let tls_config = Self::create_tls_config()?;

        let mut transport = TransportConfig::default();
        transport.max_idle_timeout(Some(
            Duration::from_secs(60)
                .try_into()
                .map_err(|e| StormError::Protocol(format!("Invalid timeout: {:?}", e)))?,
        ));
        transport.initial_mtu(1200);
        transport.min_mtu(1200);
        transport.keep_alive_interval(Some(Duration::from_secs(10)));
        transport.send_window(8 * 1024 * 1024);
        transport.receive_window(
            (8 * 1024 * 1024u64)
                .try_into()
                .map_err(|e| StormError::Protocol(format!("Invalid window: {:?}", e)))?,
        );
        transport.stream_receive_window(
            (4 * 1024 * 1024u64)
                .try_into()
                .map_err(|e| StormError::Protocol(format!("Invalid stream window: {:?}", e)))?,
        );

        let mut client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
                .map_err(|e| StormError::Protocol(format!("QUIC config error: {:?}", e)))?,
        ));
        client_config.transport_config(Arc::new(transport));

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| StormError::Network(format!("Failed to create endpoint: {}", e)))?;
        endpoint.set_default_client_config(client_config);

        Ok(Self { endpoint })
    }

    fn create_tls_config() -> Result<rustls::ClientConfig, StormError> {
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let mut tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        tls_config.alpn_protocols = vec![b"h3".to_vec()];

        Ok(tls_config)
    }

    async fn connect(
        &self,
        url: &Url,
    ) -> Result<
        (
            h3::client::SendRequest<h3_quinn::OpenStreams, bytes::Bytes>,
            Duration,
        ),
        StormError,
    > {
        let host = url
            .host_str()
            .ok_or_else(|| StormError::InvalidUrl("Missing host".into()))?;
        let port = url.port().unwrap_or(443);

        let addr = format!("{}:{}", host, port)
            .to_socket_addrs()
            .map_err(|e| StormError::Network(format!("DNS resolution failed: {}", e)))?
            .find(|a| a.is_ipv4())
            .or_else(|| {
                format!("{}:{}", host, port)
                    .to_socket_addrs()
                    .ok()
                    .and_then(|mut addrs| addrs.next())
            })
            .ok_or_else(|| StormError::Network("No addresses found for host".into()))?;

        let start = Instant::now();
        let connection = self
            .endpoint
            .connect(addr, host)
            .map_err(|e| StormError::Network(format!("Connection failed: {}", e)))?
            .await
            .map_err(|e| StormError::Network(format!("Connection error: {}", e)))?;
        let rtt = start.elapsed();

        let h3_conn = h3::client::new(h3_quinn::Connection::new(connection))
            .await
            .map_err(|e| StormError::Protocol(format!("HTTP/3 handshake failed: {}", e)))?;

        Ok((h3_conn.1, rtt))
    }

    fn build_request(&self, url: &Url, range: Option<ByteRange>) -> http::Request<()> {
        let path = if let Some(query) = url.query() {
            format!("{}?{}", url.path(), query)
        } else {
            url.path().to_string()
        };

        let mut builder = http::Request::builder()
            .method(http::Method::GET)
            .uri(&path)
            .header("host", url.host_str().unwrap_or(""))
            .header("user-agent", "StormDL/0.1");

        if let Some(r) = range {
            builder = builder.header("range", format!("bytes={}-{}", r.start, r.end - 1));
        }

        builder.body(()).unwrap()
    }
}

impl Default for Http3Downloader {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP/3 client")
    }
}

#[async_trait]
impl Downloader for Http3Downloader {
    async fn probe(&self, url: &Url) -> Result<ResourceInfo, StormError> {
        let (mut send_request, connection_rtt) = self.connect(url).await?;

        let req = self.build_request(url, Some(ByteRange::new(0, 0)));

        let mut stream = send_request
            .send_request(req)
            .await
            .map_err(|e| StormError::Network(format!("Failed to send request: {}", e)))?;

        stream
            .finish()
            .await
            .map_err(|e| StormError::Network(format!("Failed to finish request: {}", e)))?;

        let response = stream
            .recv_response()
            .await
            .map_err(|e| StormError::Network(format!("Failed to receive response: {}", e)))?;

        let status = response.status();
        if !status.is_success() && status != http::StatusCode::PARTIAL_CONTENT {
            return Err(StormError::Http {
                status: status.as_u16(),
                message: status.to_string(),
            });
        }

        let headers = response.headers();

        let (size, supports_range) = if status == http::StatusCode::PARTIAL_CONTENT {
            let size = headers
                .get(http::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').last())
                .and_then(|s| s.parse().ok());
            (size, true)
        } else {
            let size = headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            let supports_range = headers
                .get(http::header::ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v == "bytes")
                .unwrap_or(false);
            (size, supports_range)
        };

        let etag = headers
            .get(http::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let last_modified = headers
            .get(http::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let content_type = headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let filename = headers
            .get(http::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_disposition)
            .or_else(|| {
                url.path_segments()
                    .and_then(|segments| segments.last())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            });

        Ok(ResourceInfo {
            url: url.clone(),
            size,
            supports_range,
            etag,
            last_modified,
            content_type,
            filename,
            http_version: HttpVersion::Http3,
            connection_rtt: Some(connection_rtt),
        })
    }

    async fn fetch_range(
        &self,
        url: &Url,
        range: ByteRange,
        sink: &mut dyn DataSink,
    ) -> Result<(), StormError> {
        let (mut send_request, _) = self.connect(url).await?;

        let req = self.build_request(url, Some(range));

        let mut stream = send_request
            .send_request(req)
            .await
            .map_err(|e| StormError::Network(format!("Failed to send request: {}", e)))?;

        stream
            .finish()
            .await
            .map_err(|e| StormError::Network(format!("Failed to finish request: {}", e)))?;

        let response = stream
            .recv_response()
            .await
            .map_err(|e| StormError::Network(format!("Failed to receive response: {}", e)))?;

        match response.status() {
            http::StatusCode::PARTIAL_CONTENT => {}
            http::StatusCode::OK => {
                return Err(StormError::RangeNotSupported);
            }
            http::StatusCode::TOO_MANY_REQUESTS => {
                return Err(StormError::RateLimited);
            }
            status => {
                return Err(StormError::Http {
                    status: status.as_u16(),
                    message: status.to_string(),
                });
            }
        }

        while let Some(mut chunk) = stream
            .recv_data()
            .await
            .map_err(|e| StormError::Network(format!("Failed to receive data: {}", e)))?
        {
            sink.write(chunk.copy_to_bytes(chunk.remaining()))?;
        }
        sink.flush()?;

        Ok(())
    }

    async fn fetch_full(&self, url: &Url, sink: &mut dyn DataSink) -> Result<(), StormError> {
        let (mut send_request, _) = self.connect(url).await?;

        let req = self.build_request(url, None);

        let mut stream = send_request
            .send_request(req)
            .await
            .map_err(|e| StormError::Network(format!("Failed to send request: {}", e)))?;

        stream
            .finish()
            .await
            .map_err(|e| StormError::Network(format!("Failed to finish request: {}", e)))?;

        let response = stream
            .recv_response()
            .await
            .map_err(|e| StormError::Network(format!("Failed to receive response: {}", e)))?;

        if !response.status().is_success() {
            return Err(StormError::Http {
                status: response.status().as_u16(),
                message: response.status().to_string(),
            });
        }

        while let Some(mut chunk) = stream
            .recv_data()
            .await
            .map_err(|e| StormError::Network(format!("Failed to receive data: {}", e)))?
        {
            sink.write(chunk.copy_to_bytes(chunk.remaining()))?;
        }
        sink.flush()?;

        Ok(())
    }
}

fn parse_content_disposition(header: &str) -> Option<String> {
    header.split(';').find_map(|part| {
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
