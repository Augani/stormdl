use crate::{ByteRange, DownloadProgress, ResourceInfo, StormError};
use async_trait::async_trait;
use bytes::Bytes;
use std::path::Path;
use url::Url;

#[async_trait]
pub trait Downloader: Send + Sync {
    async fn probe(&self, url: &Url) -> Result<ResourceInfo, StormError>;

    async fn fetch_range(
        &self,
        url: &Url,
        range: ByteRange,
        sink: &mut dyn DataSink,
    ) -> Result<(), StormError>;

    async fn fetch_full(&self, url: &Url, sink: &mut dyn DataSink) -> Result<(), StormError>;
}

pub trait DataSink: Send {
    fn write(&mut self, data: Bytes) -> Result<(), StormError>;
    fn flush(&mut self) -> Result<(), StormError>;
}

#[async_trait]
pub trait IoBackend: Send + Sync {
    async fn create_file(&self, path: &Path, size: u64) -> Result<FileHandle, StormError>;
    async fn write_at(
        &self,
        handle: &FileHandle,
        offset: u64,
        data: &[u8],
    ) -> Result<(), StormError>;
    async fn sync(&self, handle: &FileHandle) -> Result<(), StormError>;
    async fn close(&self, handle: FileHandle) -> Result<(), StormError>;
}

#[derive(Debug)]
pub struct FileHandle {
    pub id: u64,
}

pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: DownloadProgress);
}
