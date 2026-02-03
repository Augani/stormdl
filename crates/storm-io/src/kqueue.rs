use async_trait::async_trait;
use std::path::Path;
use stormdl_core::{FileHandle, IoBackend, StormError};

pub struct KqueueBackend;

impl KqueueBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KqueueBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IoBackend for KqueueBackend {
    async fn create_file(&self, _path: &Path, _size: u64) -> Result<FileHandle, StormError> {
        Err(StormError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "KqueueBackend not yet implemented",
        )))
    }

    async fn write_at(
        &self,
        _handle: &FileHandle,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), StormError> {
        Err(StormError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "KqueueBackend not yet implemented",
        )))
    }

    async fn sync(&self, _handle: &FileHandle) -> Result<(), StormError> {
        Err(StormError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "KqueueBackend not yet implemented",
        )))
    }

    async fn close(&self, _handle: FileHandle) -> Result<(), StormError> {
        Err(StormError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "KqueueBackend not yet implemented",
        )))
    }
}
