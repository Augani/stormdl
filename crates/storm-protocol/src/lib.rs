mod http;
mod negotiation;
mod pool;

#[cfg(feature = "http3")]
mod h3;

pub use http::HttpDownloader;
pub use negotiation::{PreferredProtocol, ProtocolNegotiator};
pub use pool::ConnectionPool;

#[cfg(feature = "http3")]
pub use h3::Http3Downloader;
