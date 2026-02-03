#![allow(dead_code)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::clone_on_copy)]

use bytes::Bytes;
use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use stormdl_core::{
    ByteRange, DataSink, DownloadId, DownloadState, Downloader, SegmentState, SegmentStatus,
    StormError,
};
use stormdl_protocol::HttpDownloader;

#[cfg(feature = "gui")]
use stormdl_gui::{DownloadEvent, OrchestratorCommand};

#[cfg(not(feature = "gui"))]
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum OrchestratorCommand {
    AddDownload {
        url: url::Url,
        options: stormdl_core::DownloadOptions,
    },
    PauseDownload(DownloadId),
    ResumeDownload(DownloadId),
    CancelDownload(DownloadId),
    SetBandwidthLimit(Option<u64>),
}

#[cfg(not(feature = "gui"))]
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    DownloadAdded {
        id: DownloadId,
        url: url::Url,
        filename: String,
        total_size: Option<u64>,
    },
    ProgressUpdate {
        id: DownloadId,
        downloaded: u64,
        segments: Vec<SegmentState>,
    },
    SpeedUpdate {
        id: DownloadId,
        speed: f64,
    },
    StateChange {
        id: DownloadId,
        state: DownloadState,
    },
    SegmentRebalanced {
        id: DownloadId,
        old_count: usize,
        new_count: usize,
    },
    Error {
        id: DownloadId,
        error: String,
    },
    Complete {
        id: DownloadId,
        path: PathBuf,
        hash: String,
    },
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_download_id() -> DownloadId {
    DownloadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

struct DownloadTask {
    id: DownloadId,
    url: url::Url,
    filename: String,
    output_path: PathBuf,
    total_size: Option<u64>,
    state: DownloadState,
}

pub struct Orchestrator {
    downloads: HashMap<DownloadId, DownloadTask>,
    event_tx: Sender<DownloadEvent>,
    downloader: Arc<HttpDownloader>,
}

impl Orchestrator {
    pub fn new(event_tx: Sender<DownloadEvent>) -> Self {
        let downloader = Arc::new(HttpDownloader::new().expect("Failed to create HTTP client"));
        Self {
            downloads: HashMap::new(),
            event_tx,
            downloader,
        }
    }

    pub async fn handle_command(&mut self, cmd: OrchestratorCommand) {
        match cmd {
            OrchestratorCommand::AddDownload { url, options } => {
                self.add_download(url, options).await;
            }
            OrchestratorCommand::PauseDownload(id) => {
                self.pause_download(id).await;
            }
            OrchestratorCommand::ResumeDownload(id) => {
                self.resume_download(id).await;
            }
            OrchestratorCommand::CancelDownload(id) => {
                self.cancel_download(id).await;
            }
            OrchestratorCommand::SetBandwidthLimit(_) => {}
        }
    }

    async fn add_download(&mut self, url: url::Url, options: stormdl_core::DownloadOptions) {
        let id = next_download_id();
        let event_tx = self.event_tx.clone();
        let downloader = self.downloader.clone();

        let filename = options.filename.clone().unwrap_or_else(|| {
            url.path_segments()
                .and_then(|mut s| s.next_back())
                .unwrap_or("download")
                .to_string()
        });

        let output_path = options.output_dir.join(&filename);

        let task = DownloadTask {
            id,
            url: url.clone(),
            filename: filename.clone(),
            output_path: output_path.clone(),
            total_size: None,
            state: DownloadState::Pending,
        };

        self.downloads.insert(id, task);

        let _ = event_tx.send(DownloadEvent::DownloadAdded {
            id,
            url: url.clone(),
            filename: filename.clone(),
            total_size: None,
        });

        tokio::spawn(async move {
            run_download(id, url, output_path, downloader, event_tx).await;
        });
    }

    async fn pause_download(&mut self, id: DownloadId) {
        if let Some(task) = self.downloads.get_mut(&id) {
            task.state = DownloadState::Paused;
            let _ = self.event_tx.send(DownloadEvent::StateChange {
                id,
                state: DownloadState::Paused,
            });
        }
    }

    async fn resume_download(&mut self, id: DownloadId) {
        if let Some(task) = self.downloads.get_mut(&id) {
            task.state = DownloadState::Downloading;
            let _ = self.event_tx.send(DownloadEvent::StateChange {
                id,
                state: DownloadState::Downloading,
            });
        }
    }

    async fn cancel_download(&mut self, id: DownloadId) {
        if let Some(task) = self.downloads.get_mut(&id) {
            task.state = DownloadState::Cancelled;
            let _ = self.event_tx.send(DownloadEvent::StateChange {
                id,
                state: DownloadState::Cancelled,
            });
        }
    }
}

async fn run_download(
    id: DownloadId,
    url: url::Url,
    output_path: PathBuf,
    downloader: Arc<HttpDownloader>,
    event_tx: Sender<DownloadEvent>,
) {
    let _ = event_tx.send(DownloadEvent::StateChange {
        id,
        state: DownloadState::Probing,
    });

    let info = match downloader.probe(&url).await {
        Ok(info) => info,
        Err(e) => {
            let _ = event_tx.send(DownloadEvent::Error {
                id,
                error: e.to_string(),
            });
            return;
        }
    };

    let total_size = info.size.unwrap_or(0);
    let num_segments = if info.supports_range && total_size > 0 {
        stormdl_segment::initial_segments(total_size)
    } else {
        1
    };

    let _ = event_tx.send(DownloadEvent::DownloadAdded {
        id,
        url: url.clone(),
        filename: info
            .filename
            .clone()
            .unwrap_or_else(|| "download".to_string()),
        total_size: Some(total_size),
    });

    let _ = event_tx.send(DownloadEvent::StateChange {
        id,
        state: DownloadState::Downloading,
    });

    if let Err(e) = std::fs::File::create(&output_path).and_then(|f| f.set_len(total_size)) {
        let _ = event_tx.send(DownloadEvent::Error {
            id,
            error: format!("Failed to create file: {}", e),
        });
        return;
    }

    let segments: Vec<SegmentState> = stormdl_segment::split_range(total_size, num_segments)
        .iter()
        .enumerate()
        .map(|(idx, range)| SegmentState::new(idx, *range))
        .collect();

    let downloaded = Arc::new(AtomicU64::new(0));
    let segment_downloaded: Vec<Arc<AtomicU64>> = segments
        .iter()
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    let progress_tx = event_tx.clone();
    let progress_downloaded = downloaded.clone();
    let progress_segment_downloaded = segment_downloaded.clone();
    let progress_segments = segments.clone();

    let progress_handle = tokio::spawn(async move {
        let mut last_bytes = 0u64;
        let mut last_time = Instant::now();

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let current = progress_downloaded.load(Ordering::Relaxed);
            if current >= total_size {
                break;
            }

            let now = Instant::now();
            let interval = now.duration_since(last_time).as_secs_f64();
            let speed = if interval > 0.0 {
                (current - last_bytes) as f64 / interval
            } else {
                0.0
            };

            let segment_states: Vec<SegmentState> = progress_segments
                .iter()
                .enumerate()
                .map(|(idx, seg)| {
                    let dl = progress_segment_downloaded[idx].load(Ordering::Relaxed);
                    SegmentState {
                        id: seg.id,
                        range: seg.range,
                        downloaded: dl,
                        status: if dl >= seg.range.len() {
                            SegmentStatus::Complete
                        } else if dl > 0 {
                            SegmentStatus::Active
                        } else {
                            SegmentStatus::Pending
                        },
                        speed: 0.0,
                    }
                })
                .collect();

            let _ = progress_tx.send(DownloadEvent::ProgressUpdate {
                id,
                downloaded: current,
                segments: segment_states,
            });

            let _ = progress_tx.send(DownloadEvent::SpeedUpdate { id, speed });

            last_bytes = current;
            last_time = now;
        }
    });

    let mut handles = Vec::new();

    for (idx, segment) in segments.iter().enumerate() {
        let url = url.clone();
        let path = output_path.clone();
        let dl = downloader.clone();
        let global_downloaded = downloaded.clone();
        let seg_downloaded = segment_downloaded[idx].clone();
        let range = segment.range;

        let handle = tokio::spawn(async move {
            download_segment(dl, &url, &path, range, global_downloaded, seg_downloaded).await
        });

        handles.push(handle);
    }

    let mut has_error = false;
    for handle in handles {
        if let Err(e) = handle.await {
            has_error = true;
            let _ = event_tx.send(DownloadEvent::Error {
                id,
                error: format!("Task error: {}", e),
            });
        }
    }

    progress_handle.abort();

    if has_error {
        let _ = event_tx.send(DownloadEvent::StateChange {
            id,
            state: DownloadState::Failed,
        });
    } else {
        let final_downloaded = downloaded.load(Ordering::Relaxed);
        let segment_states: Vec<SegmentState> = segments
            .iter()
            .map(|seg| SegmentState {
                id: seg.id,
                range: seg.range,
                downloaded: seg.range.len(),
                status: SegmentStatus::Complete,
                speed: 0.0,
            })
            .collect();

        let _ = event_tx.send(DownloadEvent::ProgressUpdate {
            id,
            downloaded: final_downloaded,
            segments: segment_states,
        });

        let _ = event_tx.send(DownloadEvent::Complete {
            id,
            path: output_path,
            hash: String::new(),
        });
    }
}

async fn download_segment(
    downloader: Arc<HttpDownloader>,
    url: &url::Url,
    path: &PathBuf,
    range: ByteRange,
    global_downloaded: Arc<AtomicU64>,
    segment_downloaded: Arc<AtomicU64>,
) -> Result<(), StormError> {
    let mut file = File::options()
        .write(true)
        .open(path)
        .map_err(|e| StormError::Io(e))?;

    file.seek(SeekFrom::Start(range.start))
        .map_err(|e| StormError::Io(e))?;

    let mut sink = ProgressSink {
        file,
        global_downloaded,
        segment_downloaded,
    };

    downloader.fetch_range(url, range, &mut sink).await?;
    sink.file.flush().map_err(|e| StormError::Io(e))?;

    Ok(())
}

struct ProgressSink {
    file: File,
    global_downloaded: Arc<AtomicU64>,
    segment_downloaded: Arc<AtomicU64>,
}

impl DataSink for ProgressSink {
    fn write(&mut self, data: Bytes) -> Result<(), StormError> {
        self.file.write_all(&data).map_err(|e| StormError::Io(e))?;
        let len = data.len() as u64;
        self.global_downloaded.fetch_add(len, Ordering::Relaxed);
        self.segment_downloaded.fetch_add(len, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), StormError> {
        self.file.flush().map_err(|e| StormError::Io(e))
    }
}

pub async fn run(cmd_rx: Receiver<OrchestratorCommand>, event_tx: Sender<DownloadEvent>) {
    let mut orchestrator = Orchestrator::new(event_tx);

    while let Ok(cmd) = cmd_rx.recv_async().await {
        orchestrator.handle_command(cmd).await;
    }
}
