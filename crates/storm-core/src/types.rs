use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

fn duration_millis_opt<S>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match d {
        Some(dur) => s.serialize_some(&dur.as_millis()),
        None => s.serialize_none(),
    }
}

fn duration_millis_opt_de<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<u128> = Option::deserialize(d)?;
    Ok(opt.map(|ms| Duration::from_millis(ms as u64)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloadId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn split_at(&self, offset: u64) -> (ByteRange, ByteRange) {
        let mid = self.start + offset;
        (
            ByteRange::new(self.start, mid),
            ByteRange::new(mid, self.end),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpVersion {
    Http1_1,
    Http2,
    Http3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub url: Url,
    pub size: Option<u64>,
    pub supports_range: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub filename: Option<String>,
    pub http_version: HttpVersion,
    #[serde(
        serialize_with = "duration_millis_opt",
        deserialize_with = "duration_millis_opt_de",
        default
    )]
    pub connection_rtt: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    Pending,
    Probing,
    Downloading,
    Paused,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentStatus {
    Pending,
    Active,
    Complete,
    Error,
    Slow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentState {
    pub id: usize,
    pub range: ByteRange,
    pub downloaded: u64,
    pub status: SegmentStatus,
    pub speed: f64,
}

impl SegmentState {
    pub fn new(id: usize, range: ByteRange) -> Self {
        Self {
            id,
            range,
            downloaded: 0,
            status: SegmentStatus::Pending,
            speed: 0.0,
        }
    }

    pub fn remaining(&self) -> u64 {
        self.range.len().saturating_sub(self.downloaded)
    }

    pub fn progress(&self) -> f64 {
        if self.range.is_empty() {
            return 1.0;
        }
        self.downloaded as f64 / self.range.len() as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
    Background,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadOptions {
    pub url: Url,
    pub output_dir: PathBuf,
    pub filename: Option<String>,
    pub segments: Option<usize>,
    pub priority: Priority,
    pub bandwidth_limit: Option<u64>,
    pub headers: Vec<(String, String)>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub id: DownloadId,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: f64,
    pub eta: Option<Duration>,
    pub segments: Vec<SegmentState>,
    pub state: DownloadState,
}
