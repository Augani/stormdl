use async_trait::async_trait;
use std::path::Path;
use storm_core::{FileHandle, IoBackend, StormError};

pub struct UringBackend;

impl UringBackend {
    pub fn new() -> Result<Self, StormError> {
        Ok(Self)
    }
}

impl Default for UringBackend {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl IoBackend for UringBackend {
    async fn create_file(&self, path: &Path, size: u64) -> Result<FileHandle, StormError> {
        use tokio::fs::OpenOptions;

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;

        file.set_len(size).await?;
        let id = path.as_os_str().len() as u64;
        Ok(FileHandle { id })
    }

    async fn write_at(
        &self,
        _handle: &FileHandle,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), StormError> {
        Err(StormError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "io_uring not yet implemented",
        )))
    }

    async fn sync(&self, _handle: &FileHandle) -> Result<(), StormError> {
        Ok(())
    }

    async fn close(&self, _handle: FileHandle) -> Result<(), StormError> {
        Ok(())
    }
}
