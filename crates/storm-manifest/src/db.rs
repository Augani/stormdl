use rusqlite::{Connection, Result as SqlResult, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use storm_core::{ByteRange, DownloadState, StormError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: i64,
    pub url: String,
    pub filename: String,
    pub output_path: PathBuf,
    pub total_size: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub state: DownloadState,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentEntry {
    pub id: i64,
    pub download_id: i64,
    pub segment_index: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded_bytes: u64,
    pub hash: Option<String>,
    pub complete: bool,
}

impl SegmentEntry {
    pub fn range(&self) -> ByteRange {
        ByteRange::new(self.start_byte, self.end_byte)
    }
}

pub struct Manifest {
    conn: Connection,
}

impl Manifest {
    pub fn open(path: &Path) -> Result<Self, StormError> {
        let conn = Connection::open(path).map_err(|e| StormError::Database(e.to_string()))?;

        let manifest = Self { conn };
        manifest.init_schema()?;

        Ok(manifest)
    }

    pub fn open_in_memory() -> Result<Self, StormError> {
        let conn = Connection::open_in_memory().map_err(|e| StormError::Database(e.to_string()))?;

        let manifest = Self { conn };
        manifest.init_schema()?;

        Ok(manifest)
    }

    fn init_schema(&self) -> Result<(), StormError> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                output_path TEXT NOT NULL,
                total_size INTEGER,
                etag TEXT,
                last_modified TEXT,
                state TEXT NOT NULL DEFAULT 'Pending',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                download_id INTEGER NOT NULL,
                segment_index INTEGER NOT NULL,
                start_byte INTEGER NOT NULL,
                end_byte INTEGER NOT NULL,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                hash TEXT,
                complete INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (download_id) REFERENCES downloads(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_segments_download ON segments(download_id);
            ",
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn create_download(
        &self,
        url: &str,
        filename: &str,
        output_path: &Path,
        total_size: Option<u64>,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<i64, StormError> {
        self.conn
            .execute(
                "INSERT INTO downloads (url, filename, output_path, total_size, etag, last_modified)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    url,
                    filename,
                    output_path.to_string_lossy(),
                    total_size,
                    etag,
                    last_modified
                ],
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_segment(
        &self,
        download_id: i64,
        segment_index: usize,
        range: ByteRange,
    ) -> Result<i64, StormError> {
        self.conn
            .execute(
                "INSERT INTO segments (download_id, segment_index, start_byte, end_byte)
                 VALUES (?1, ?2, ?3, ?4)",
                params![download_id, segment_index, range.start, range.end],
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_segment_progress(
        &self,
        segment_id: i64,
        downloaded_bytes: u64,
        hash: Option<&str>,
    ) -> Result<(), StormError> {
        self.conn
            .execute(
                "UPDATE segments SET downloaded_bytes = ?1, hash = ?2 WHERE id = ?3",
                params![downloaded_bytes, hash, segment_id],
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn mark_segment_complete(&self, segment_id: i64, hash: &str) -> Result<(), StormError> {
        self.conn
            .execute(
                "UPDATE segments SET complete = 1, hash = ?1 WHERE id = ?2",
                params![hash, segment_id],
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn update_download_state(
        &self,
        download_id: i64,
        state: DownloadState,
    ) -> Result<(), StormError> {
        let state_str = format!("{:?}", state);
        self.conn
            .execute(
                "UPDATE downloads SET state = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![state_str, download_id],
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn get_download(&self, download_id: i64) -> Result<Option<ManifestEntry>, StormError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, url, filename, output_path, total_size, etag, last_modified, state, created_at, updated_at
                 FROM downloads WHERE id = ?1",
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        let result = stmt
            .query_row(params![download_id], |row| {
                Ok(ManifestEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    filename: row.get(2)?,
                    output_path: PathBuf::from(row.get::<_, String>(3)?),
                    total_size: row.get(4)?,
                    etag: row.get(5)?,
                    last_modified: row.get(6)?,
                    state: parse_state(&row.get::<_, String>(7)?),
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .optional()
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(result)
    }

    pub fn get_segments(&self, download_id: i64) -> Result<Vec<SegmentEntry>, StormError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, download_id, segment_index, start_byte, end_byte, downloaded_bytes, hash, complete
                 FROM segments WHERE download_id = ?1 ORDER BY segment_index",
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        let segments = stmt
            .query_map(params![download_id], |row| {
                Ok(SegmentEntry {
                    id: row.get(0)?,
                    download_id: row.get(1)?,
                    segment_index: row.get(2)?,
                    start_byte: row.get(3)?,
                    end_byte: row.get(4)?,
                    downloaded_bytes: row.get(5)?,
                    hash: row.get(6)?,
                    complete: row.get(7)?,
                })
            })
            .map_err(|e| StormError::Database(e.to_string()))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(segments)
    }

    pub fn get_incomplete_downloads(&self) -> Result<Vec<ManifestEntry>, StormError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, url, filename, output_path, total_size, etag, last_modified, state, created_at, updated_at
                 FROM downloads WHERE state NOT IN ('Complete', 'Cancelled')",
            )
            .map_err(|e| StormError::Database(e.to_string()))?;

        let downloads = stmt
            .query_map([], |row| {
                Ok(ManifestEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    filename: row.get(2)?,
                    output_path: PathBuf::from(row.get::<_, String>(3)?),
                    total_size: row.get(4)?,
                    etag: row.get(5)?,
                    last_modified: row.get(6)?,
                    state: parse_state(&row.get::<_, String>(7)?),
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .map_err(|e| StormError::Database(e.to_string()))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(downloads)
    }

    pub fn delete_download(&self, download_id: i64) -> Result<(), StormError> {
        self.conn
            .execute("DELETE FROM downloads WHERE id = ?1", params![download_id])
            .map_err(|e| StormError::Database(e.to_string()))?;

        Ok(())
    }
}

fn parse_state(s: &str) -> DownloadState {
    match s {
        "Pending" => DownloadState::Pending,
        "Probing" => DownloadState::Probing,
        "Downloading" => DownloadState::Downloading,
        "Paused" => DownloadState::Paused,
        "Complete" => DownloadState::Complete,
        "Failed" => DownloadState::Failed,
        "Cancelled" => DownloadState::Cancelled,
        _ => DownloadState::Pending,
    }
}

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
