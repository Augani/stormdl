mod coalesce;

#[cfg(target_os = "linux")]
mod uring;

#[cfg(target_os = "macos")]
mod kqueue;

#[cfg(target_os = "windows")]
mod iocp;

pub use coalesce::WriteBuffer;

#[cfg(target_os = "linux")]
pub use uring::UringBackend;

#[cfg(target_os = "macos")]
pub use kqueue::KqueueBackend;

#[cfg(target_os = "windows")]
pub use iocp::IocpBackend;

use async_trait::async_trait;
use std::path::Path;
use storm_core::{FileHandle, IoBackend, StormError};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

pub struct TokioBackend;

impl TokioBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TokioBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IoBackend for TokioBackend {
    async fn create_file(&self, path: &Path, size: u64) -> Result<FileHandle, StormError> {
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
            "TokioBackend requires file path for writes",
        )))
    }

    async fn sync(&self, _handle: &FileHandle) -> Result<(), StormError> {
        Ok(())
    }

    async fn close(&self, _handle: FileHandle) -> Result<(), StormError> {
        Ok(())
    }
}

pub struct FileWriter {
    file: File,
    buffer: WriteBuffer,
}

impl FileWriter {
    pub async fn new(path: &Path, size: u64, buffer_size: usize) -> Result<Self, StormError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;

        file.set_len(size).await?;

        Ok(Self {
            file,
            buffer: WriteBuffer::new(buffer_size),
        })
    }

    pub async fn write(&mut self, data: &[u8]) -> Result<(), StormError> {
        if self.buffer.would_overflow(data.len()) {
            self.flush().await?;
        }
        self.buffer.append(data);
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), StormError> {
        if !self.buffer.is_empty() {
            self.file.write_all(self.buffer.data()).await?;
            self.buffer.clear();
        }
        Ok(())
    }

    pub async fn sync(&mut self) -> Result<(), StormError> {
        self.flush().await?;
        self.file.sync_all().await?;
        Ok(())
    }
}
