use flume::{Receiver, Sender};
use smallvec::SmallVec;
use std::path::PathBuf;
use storm_core::{DownloadId, DownloadOptions, DownloadState, SegmentState};
use url::Url;

#[derive(Debug, Clone)]
pub enum OrchestratorCommand {
    AddDownload { url: Url, options: DownloadOptions },
    PauseDownload(DownloadId),
    ResumeDownload(DownloadId),
    CancelDownload(DownloadId),
    SetBandwidthLimit(Option<u64>),
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    DownloadAdded {
        id: DownloadId,
        url: Url,
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

#[derive(Debug, Clone)]
pub struct Download {
    pub id: DownloadId,
    pub url: Url,
    pub filename: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub state: DownloadState,
    pub segments: Vec<SegmentState>,
    pub speed_samples: SmallVec<[f64; 30]>,
    pub error: Option<String>,
}

impl Download {
    pub fn new(id: DownloadId, url: Url, filename: String, total_bytes: Option<u64>) -> Self {
        Self {
            id,
            url,
            filename,
            total_bytes,
            downloaded_bytes: 0,
            state: DownloadState::Pending,
            segments: Vec::new(),
            speed_samples: SmallVec::new(),
            error: None,
        }
    }

    pub fn progress(&self) -> f64 {
        match self.total_bytes {
            Some(total) if total > 0 => self.downloaded_bytes as f64 / total as f64,
            _ => 0.0,
        }
    }

    pub fn current_speed(&self) -> f64 {
        self.speed_samples.last().copied().unwrap_or(0.0)
    }

    pub fn average_speed(&self) -> f64 {
        if self.speed_samples.is_empty() {
            return 0.0;
        }
        self.speed_samples.iter().sum::<f64>() / self.speed_samples.len() as f64
    }

    pub fn add_speed_sample(&mut self, speed: f64) {
        if self.speed_samples.len() >= 30 {
            self.speed_samples.remove(0);
        }
        self.speed_samples.push(speed);
    }
}

pub struct AppState {
    pub downloads: Vec<Download>,
    pub selected_download_id: Option<DownloadId>,
    pub command_tx: Sender<OrchestratorCommand>,
    pub event_rx: Receiver<DownloadEvent>,
    pub settings: Settings,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub max_concurrent: usize,
    pub max_segments: usize,
    pub bandwidth_limit: Option<u64>,
    pub turbo_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            max_concurrent: 3,
            max_segments: 32,
            bandwidth_limit: None,
            turbo_mode: false,
        }
    }
}

impl AppState {
    pub fn new(command_tx: Sender<OrchestratorCommand>, event_rx: Receiver<DownloadEvent>) -> Self {
        Self {
            downloads: Vec::new(),
            selected_download_id: None,
            command_tx,
            event_rx,
            settings: Settings::default(),
        }
    }

    pub fn add_download(
        &mut self,
        id: DownloadId,
        url: Url,
        filename: String,
        total_bytes: Option<u64>,
    ) {
        let download = Download::new(id, url, filename, total_bytes);
        self.downloads.push(download);
    }

    pub fn get_download(&self, id: DownloadId) -> Option<&Download> {
        self.downloads.iter().find(|d| d.id == id)
    }

    pub fn get_download_mut(&mut self, id: DownloadId) -> Option<&mut Download> {
        self.downloads.iter_mut().find(|d| d.id == id)
    }

    pub fn remove_download(&mut self, id: DownloadId) {
        self.downloads.retain(|d| d.id != id);
        if self.selected_download_id == Some(id) {
            self.selected_download_id = None;
        }
    }
}
