use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use storm_core::{DownloadId, DownloadState, SegmentState};

#[cfg(feature = "gui")]
use storm_gui::{DownloadEvent, OrchestratorCommand};

#[cfg(not(feature = "gui"))]
#[derive(Debug, Clone)]
pub enum OrchestratorCommand {
    AddDownload { url: url::Url, options: storm_core::DownloadOptions },
    PauseDownload(DownloadId),
    ResumeDownload(DownloadId),
    CancelDownload(DownloadId),
    SetBandwidthLimit(Option<u64>),
}

#[cfg(not(feature = "gui"))]
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    DownloadAdded { id: DownloadId, url: url::Url, filename: String, total_size: Option<u64> },
    ProgressUpdate { id: DownloadId, downloaded: u64, segments: Vec<SegmentState> },
    SpeedUpdate { id: DownloadId, speed: f64 },
    StateChange { id: DownloadId, state: DownloadState },
    SegmentRebalanced { id: DownloadId, old_count: usize, new_count: usize },
    Error { id: DownloadId, error: String },
    Complete { id: DownloadId, path: PathBuf, hash: String },
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
    bandwidth_limit: Option<u64>,
}

impl Orchestrator {
    pub fn new(event_tx: Sender<DownloadEvent>) -> Self {
        Self {
            downloads: HashMap::new(),
            event_tx,
            bandwidth_limit: None,
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
            OrchestratorCommand::SetBandwidthLimit(limit) => {
                self.bandwidth_limit = limit;
            }
        }
    }

    async fn add_download(&mut self, url: url::Url, options: storm_core::DownloadOptions) {
        let id = next_download_id();
        let filename = options.filename
            .clone()
            .unwrap_or_else(|| {
                url.path_segments()
                    .and_then(|s| s.last())
                    .unwrap_or("download")
                    .to_string()
            });

        let output_path = options.output_dir.join(&filename);

        let task = DownloadTask {
            id,
            url: url.clone(),
            filename: filename.clone(),
            output_path,
            total_size: None,
            state: DownloadState::Pending,
        };

        self.downloads.insert(id, task);

        let _ = self.event_tx.send(DownloadEvent::DownloadAdded {
            id,
            url,
            filename,
            total_size: None,
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

pub async fn run(cmd_rx: Receiver<OrchestratorCommand>, event_tx: Sender<DownloadEvent>) {
    let mut orchestrator = Orchestrator::new(event_tx);

    while let Ok(cmd) = cmd_rx.recv_async().await {
        orchestrator.handle_command(cmd).await;
    }
}
