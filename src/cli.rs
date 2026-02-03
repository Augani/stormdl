use anyhow::{Context, Result};
use bytes::Bytes;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use storm_core::{ByteRange, Downloader, ResourceInfo};
use storm_protocol::HttpDownloader;
use storm_segment::SegmentManager;
use tokio::sync::Notify;
use url::Url;

pub struct DownloadArgs {
    pub output: Option<String>,
    pub name: Option<String>,
    pub segments: Option<usize>,
    pub limit: Option<String>,
    pub turbo: bool,
    pub no_resume: bool,
    pub checksum: Option<String>,
    pub quiet: bool,
    pub mirrors: Vec<String>,
}

struct SegmentTracker {
    downloaded: AtomicU64,
    total: u64,
    remaining_start: AtomicU64,
    last_progress: Mutex<(u64, Instant)>,
    active: AtomicBool,
}

impl SegmentTracker {
    fn new(total: u64, start: u64) -> Self {
        Self {
            downloaded: AtomicU64::new(0),
            total,
            remaining_start: AtomicU64::new(start),
            last_progress: Mutex::new((0, Instant::now())),
            active: AtomicBool::new(true),
        }
    }

    fn speed(&self) -> f64 {
        let (last_bytes, last_time) = *self.last_progress.lock();
        let current = self.downloaded.load(Ordering::Relaxed);
        let elapsed = last_time.elapsed().as_secs_f64();
        if elapsed > 0.5 {
            (current.saturating_sub(last_bytes)) as f64 / elapsed
        } else {
            0.0
        }
    }

    fn update_speed_sample(&self) {
        let current = self.downloaded.load(Ordering::Relaxed);
        *self.last_progress.lock() = (current, Instant::now());
    }

    fn remaining(&self) -> u64 {
        self.total
            .saturating_sub(self.downloaded.load(Ordering::Relaxed))
    }

    fn is_complete(&self) -> bool {
        self.downloaded.load(Ordering::Relaxed) >= self.total
    }
}

struct WorkQueue {
    ranges: Mutex<VecDeque<(ByteRange, usize)>>,
    notify: Notify,
}

impl WorkQueue {
    fn new() -> Self {
        Self {
            ranges: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
        }
    }

    fn push(&self, range: ByteRange, segment_idx: usize) {
        self.ranges.lock().push_back((range, segment_idx));
        self.notify.notify_one();
    }

    fn pop(&self) -> Option<(ByteRange, usize)> {
        self.ranges.lock().pop_front()
    }

    fn is_empty(&self) -> bool {
        self.ranges.lock().is_empty()
    }
}

struct Progress {
    total: u64,
    downloaded: Arc<AtomicU64>,
    segment_progress: Option<Arc<RwLock<Vec<(u64, u64)>>>>,
    start_time: Instant,
    last_bytes: u64,
    last_time: Instant,
    done: Arc<AtomicBool>,
    num_segments: usize,
}

impl Progress {
    fn new(total: u64, downloaded: Arc<AtomicU64>, done: Arc<AtomicBool>) -> Self {
        Self {
            total,
            downloaded,
            segment_progress: None,
            start_time: Instant::now(),
            last_bytes: 0,
            last_time: Instant::now(),
            done,
            num_segments: 1,
        }
    }

    fn with_segments(
        total: u64,
        downloaded: Arc<AtomicU64>,
        done: Arc<AtomicBool>,
        segment_progress: Arc<RwLock<Vec<(u64, u64)>>>,
        num_segments: usize,
    ) -> Self {
        Self {
            total,
            downloaded,
            segment_progress: Some(segment_progress),
            start_time: Instant::now(),
            last_bytes: 0,
            last_time: Instant::now(),
            done,
            num_segments,
        }
    }

    fn display(&mut self) {
        let current = self.downloaded.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let interval = self.last_time.elapsed().as_secs_f64();

        let speed = if interval > 0.1 {
            (current.saturating_sub(self.last_bytes)) as f64 / interval
        } else {
            0.0
        };

        let avg_speed = if elapsed > 0.0 {
            current as f64 / elapsed
        } else {
            0.0
        };

        let percent = if self.total > 0 {
            (current as f64 / self.total as f64) * 100.0
        } else {
            0.0
        };

        let eta = if avg_speed > 0.0 && self.total > current {
            let remaining = self.total - current;
            Some(Duration::from_secs_f64(remaining as f64 / avg_speed))
        } else {
            None
        };

        let bar_width = 30;
        let filled = (percent / 100.0 * bar_width as f64) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

        let eta_str = match eta {
            Some(d) => {
                let secs = d.as_secs();
                if secs >= 3600 {
                    format!(
                        "{:02}:{:02}:{:02}",
                        secs / 3600,
                        (secs % 3600) / 60,
                        secs % 60
                    )
                } else {
                    format!("{:02}:{:02}", secs / 60, secs % 60)
                }
            }
            None => "--:--".to_string(),
        };

        let segment_str = if let Some(ref seg_progress) = self.segment_progress {
            let segs = seg_progress.read();
            let indicators: String = segs
                .iter()
                .map(|(downloaded, total)| {
                    if *total == 0 {
                        '░'
                    } else if *downloaded >= *total {
                        '█'
                    } else if *downloaded > 0 {
                        '▓'
                    } else {
                        '░'
                    }
                })
                .collect();
            format!(" [{}]", indicators)
        } else {
            String::new()
        };

        eprint!(
            "\r[{}] {:5.1}% | {} / {} | {:>8}/s | ETA: {}{} ",
            bar,
            percent,
            format_bytes(current),
            format_bytes(self.total),
            format_bytes(speed as u64),
            eta_str,
            segment_str
        );
        io::stderr().flush().ok();

        if interval > 0.1 {
            self.last_bytes = current;
            self.last_time = Instant::now();
        }
    }

    fn finish(&self) {
        let current = self.downloaded.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();
        let avg_speed = if elapsed.as_secs_f64() > 0.0 {
            current as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let segment_str = if self.num_segments > 1 {
            format!(" [{}]", "█".repeat(self.num_segments))
        } else {
            String::new()
        };

        eprintln!(
            "\r[{}] 100.0% | {} | {:>8}/s | {:.1}s{}        ",
            "█".repeat(30),
            format_bytes(current),
            format_bytes(avg_speed as u64),
            elapsed.as_secs_f64(),
            segment_str
        );
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn calculate_segments(info: &ResourceInfo, args: &DownloadArgs) -> usize {
    let total_size = info.size.unwrap_or(0);

    if let Some(s) = args.segments {
        return s;
    }

    if let Some(rtt) = info.connection_rtt {
        let estimated_bandwidth = 10_000_000.0;
        let optimal = storm_segment::optimal_segments(total_size, estimated_bandwidth, rtt);
        if args.turbo {
            (optimal * 2).min(32)
        } else {
            optimal
        }
    } else if args.turbo {
        storm_segment::turbo_segments(total_size)
    } else {
        storm_segment::initial_segments(total_size)
    }
}

pub fn download(url_str: &str, args: DownloadArgs) -> Result<()> {
    let url = Url::parse(url_str).context("Invalid URL")?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move { download_async(url, args).await })
}

async fn download_async(url: Url, args: DownloadArgs) -> Result<()> {
    let downloader = if args.turbo {
        HttpDownloader::turbo()?
    } else {
        HttpDownloader::new()?
    };

    if !args.quiet {
        eprintln!("Probing {}...", url);
    }

    let info = downloader.probe(&url).await?;

    let total_size = info.size.unwrap_or(0);
    let num_segments = calculate_segments(&info, &args);

    let filename = args
        .name
        .or(info.filename.clone())
        .unwrap_or_else(|| "download".to_string());

    let output_dir = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")));

    let output_path = output_dir.join(&filename);

    if !args.quiet {
        eprintln!("Filename: {}", filename);
        eprintln!("Size: {}", format_bytes(total_size));
        if let Some(rtt) = info.connection_rtt {
            eprintln!("RTT: {:.1}ms", rtt.as_secs_f64() * 1000.0);
        }
        let mode_str = if args.segments.is_some() {
            " (manual)"
        } else if info.connection_rtt.is_some() {
            " (BDP-optimized)"
        } else if args.turbo {
            ""
        } else {
            " (gentle)"
        };
        eprintln!("Segments: {}{}", num_segments, mode_str);
        eprintln!("Output: {}", output_path.display());
        eprintln!();
    }

    if !info.supports_range || total_size == 0 {
        download_single(&downloader, &url, &output_path, total_size, args.quiet).await?;
    } else {
        download_segmented_adaptive(
            &url,
            &output_path,
            total_size,
            num_segments,
            args.quiet,
            args.turbo,
        )
        .await?;
    }

    if !args.quiet {
        eprintln!("Download complete: {}", output_path.display());
    }

    if let Some(expected_hash) = args.checksum {
        if !args.quiet {
            eprintln!("Verifying checksum...");
        }
        let data = tokio::fs::read(&output_path).await?;
        let mut hasher = storm_integrity::IncrementalHasher::new();
        hasher.update(&data);
        let actual_hash = hasher.finalize();

        if actual_hash != expected_hash {
            anyhow::bail!(
                "Checksum mismatch: expected {}, got {}",
                expected_hash,
                actual_hash
            );
        }

        if !args.quiet {
            eprintln!("Checksum verified: {}", actual_hash);
        }
    }

    Ok(())
}

async fn download_single(
    downloader: &HttpDownloader,
    url: &Url,
    output_path: &PathBuf,
    total_size: u64,
    quiet: bool,
) -> Result<()> {
    let downloaded = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let progress_downloaded = downloaded.clone();
    let progress_done = done.clone();

    let progress_handle = if !quiet && total_size > 0 {
        Some(tokio::spawn(async move {
            let mut progress =
                Progress::new(total_size, progress_downloaded, progress_done.clone());
            while !progress_done.load(Ordering::Relaxed) {
                progress.display();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            progress.finish();
        }))
    } else {
        None
    };

    let mut sink = ProgressFileSink::new(output_path, downloaded.clone())?;
    downloader.fetch_full(url, &mut sink).await?;
    sink.flush()?;

    done.store(true, Ordering::Relaxed);
    if let Some(handle) = progress_handle {
        handle.await?;
    }

    Ok(())
}

async fn download_segmented_adaptive(
    url: &Url,
    output_path: &PathBuf,
    total_size: u64,
    num_segments: usize,
    quiet: bool,
    turbo: bool,
) -> Result<()> {
    let manager = Arc::new(SegmentManager::with_segments(total_size, num_segments));
    let segments = manager.get_segments();

    {
        let file = File::create(output_path)?;
        file.set_len(total_size)?;
    }

    let downloader = Arc::new(if turbo {
        HttpDownloader::turbo()?
    } else {
        HttpDownloader::new()?
    });

    let downloaded = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let segment_progress: Arc<RwLock<Vec<(u64, u64)>>> = Arc::new(RwLock::new(
        segments.iter().map(|s| (0u64, s.range.len())).collect(),
    ));

    let trackers: Arc<Vec<Arc<SegmentTracker>>> = Arc::new(
        segments
            .iter()
            .map(|s| Arc::new(SegmentTracker::new(s.range.len(), s.range.start)))
            .collect(),
    );

    let work_queue = Arc::new(WorkQueue::new());

    for (idx, segment) in segments.iter().enumerate() {
        work_queue.push(segment.range.clone(), idx);
    }

    let progress_downloaded = downloaded.clone();
    let progress_done = done.clone();
    let progress_segments = segment_progress.clone();

    let progress_handle = if !quiet {
        Some(tokio::spawn(async move {
            let mut progress = Progress::with_segments(
                total_size,
                progress_downloaded,
                progress_done.clone(),
                progress_segments,
                num_segments,
            );
            while !progress_done.load(Ordering::Relaxed) {
                progress.display();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            progress.finish();
        }))
    } else {
        None
    };

    let rebalance_done = done.clone();
    let rebalance_trackers = trackers.clone();
    let rebalance_queue = work_queue.clone();
    let rebalance_segments = segments.clone();

    let rebalance_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;

        while !rebalance_done.load(Ordering::Relaxed) {
            for tracker in rebalance_trackers.iter() {
                tracker.update_speed_sample();
            }

            let speeds: Vec<f64> = rebalance_trackers.iter().map(|t| t.speed()).collect();
            let active_speeds: Vec<f64> = speeds
                .iter()
                .zip(rebalance_trackers.iter())
                .filter(|(_, t)| t.active.load(Ordering::Relaxed) && !t.is_complete())
                .map(|(s, _)| *s)
                .collect();

            if active_speeds.len() > 1 {
                let avg_speed: f64 = active_speeds.iter().sum::<f64>() / active_speeds.len() as f64;
                let threshold = avg_speed * 0.3;

                for (idx, (tracker, segment)) in rebalance_trackers
                    .iter()
                    .zip(rebalance_segments.iter())
                    .enumerate()
                {
                    let speed = speeds[idx];
                    let remaining = tracker.remaining();

                    if speed > 0.0
                        && speed < threshold
                        && remaining > 512 * 1024
                        && tracker.active.load(Ordering::Relaxed)
                    {
                        let current_pos = tracker.remaining_start.load(Ordering::Relaxed)
                            + tracker.downloaded.load(Ordering::Relaxed);
                        let end = segment.range.end;

                        if end > current_pos + 256 * 1024 {
                            let split_point = current_pos + (end - current_pos) / 2;
                            let steal_range = ByteRange::new(split_point, end);
                            tracker
                                .remaining_start
                                .store(split_point, Ordering::Relaxed);

                            rebalance_queue.push(steal_range, idx);
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    let max_workers = if turbo {
        num_segments + 8
    } else {
        num_segments + 4
    };
    let active_workers = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    for _ in 0..num_segments {
        let url = url.clone();
        let path = output_path.clone();
        let downloaded = downloaded.clone();
        let seg_progress = segment_progress.clone();
        let dl = downloader.clone();
        let queue = work_queue.clone();
        let trks = trackers.clone();
        let workers = active_workers.clone();
        let all_done = done.clone();

        workers.fetch_add(1, Ordering::Relaxed);

        let handle = tokio::spawn(async move {
            loop {
                let work = queue.pop();
                match work {
                    Some((range, seg_idx)) => {
                        let result = download_range(
                            dl.clone(),
                            &url,
                            &path,
                            range,
                            downloaded.clone(),
                            seg_progress.clone(),
                            trks.clone(),
                            seg_idx,
                        )
                        .await;

                        if let Err(e) = result {
                            tracing::error!("Segment {} error: {}", seg_idx, e);
                        }
                    }
                    None => {
                        if all_done.load(Ordering::Relaxed) || queue.is_empty() {
                            let all_complete = trks.iter().all(|t| t.is_complete());
                            if all_complete {
                                break;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
            workers.fetch_sub(1, Ordering::Relaxed);
        });

        handles.push(handle);
    }

    let spawn_done = done.clone();
    let spawn_queue = work_queue.clone();
    let spawn_trackers = trackers.clone();
    let spawn_workers = active_workers.clone();
    let spawn_downloaded = downloaded.clone();
    let spawn_seg_progress = segment_progress.clone();
    let spawn_downloader = downloader.clone();
    let spawn_url = url.clone();
    let spawn_path = output_path.clone();

    let spawner_handle = tokio::spawn(async move {
        while !spawn_done.load(Ordering::Relaxed) {
            let current_workers = spawn_workers.load(Ordering::Relaxed) as usize;
            let has_work = !spawn_queue.is_empty();
            let all_complete = spawn_trackers.iter().all(|t| t.is_complete());

            if all_complete {
                break;
            }

            if has_work && current_workers < max_workers {
                let url = spawn_url.clone();
                let path = spawn_path.clone();
                let downloaded = spawn_downloaded.clone();
                let seg_progress = spawn_seg_progress.clone();
                let dl = spawn_downloader.clone();
                let queue = spawn_queue.clone();
                let trks = spawn_trackers.clone();
                let workers = spawn_workers.clone();
                let all_done = spawn_done.clone();

                workers.fetch_add(1, Ordering::Relaxed);

                tokio::spawn(async move {
                    loop {
                        let work = queue.pop();
                        match work {
                            Some((range, seg_idx)) => {
                                let result = download_range(
                                    dl.clone(),
                                    &url,
                                    &path,
                                    range,
                                    downloaded.clone(),
                                    seg_progress.clone(),
                                    trks.clone(),
                                    seg_idx,
                                )
                                .await;

                                if let Err(e) = result {
                                    tracing::error!("Helper segment {} error: {}", seg_idx, e);
                                }
                            }
                            None => {
                                if all_done.load(Ordering::Relaxed) {
                                    break;
                                }
                                let all_complete = trks.iter().all(|t| t.is_complete());
                                if all_complete {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        }
                    }
                    workers.fetch_sub(1, Ordering::Relaxed);
                });
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    for handle in handles {
        let _ = handle.await;
    }

    done.store(true, Ordering::Relaxed);
    let _ = rebalance_handle.await;
    let _ = spawner_handle.await;

    if let Some(handle) = progress_handle {
        handle.await?;
    }

    Ok(())
}

async fn download_range(
    downloader: Arc<HttpDownloader>,
    url: &Url,
    path: &PathBuf,
    range: ByteRange,
    global_downloaded: Arc<AtomicU64>,
    segment_progress: Arc<RwLock<Vec<(u64, u64)>>>,
    trackers: Arc<Vec<Arc<SegmentTracker>>>,
    segment_idx: usize,
) -> Result<()> {
    use std::io::{Seek, SeekFrom};

    let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(range.start))?;

    let tracker = &trackers[segment_idx];
    let range_size = range.len();

    let mut sink = AdaptiveSink {
        file,
        global_downloaded,
        segment_progress,
        segment_idx,
        tracker: tracker.clone(),
        written: 0,
    };

    downloader.fetch_range(url, range, &mut sink).await?;
    sink.file.flush()?;

    Ok(())
}

struct ProgressFileSink {
    file: File,
    downloaded: Arc<AtomicU64>,
}

impl ProgressFileSink {
    fn new(path: &PathBuf, downloaded: Arc<AtomicU64>) -> Result<Self> {
        let file = File::create(path)?;
        Ok(Self { file, downloaded })
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush()?;
        Ok(())
    }
}

impl storm_core::DataSink for ProgressFileSink {
    fn write(&mut self, data: Bytes) -> Result<(), storm_core::StormError> {
        self.file.write_all(&data)?;
        self.downloaded
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), storm_core::StormError> {
        Write::flush(&mut self.file)?;
        Ok(())
    }
}

struct AdaptiveSink {
    file: File,
    global_downloaded: Arc<AtomicU64>,
    segment_progress: Arc<RwLock<Vec<(u64, u64)>>>,
    segment_idx: usize,
    tracker: Arc<SegmentTracker>,
    written: u64,
}

impl storm_core::DataSink for AdaptiveSink {
    fn write(&mut self, data: Bytes) -> Result<(), storm_core::StormError> {
        self.file.write_all(&data)?;
        let len = data.len() as u64;
        self.global_downloaded.fetch_add(len, Ordering::Relaxed);
        self.tracker.downloaded.fetch_add(len, Ordering::Relaxed);
        self.written += len;

        {
            let mut segs = self.segment_progress.write();
            if let Some(seg) = segs.get_mut(self.segment_idx) {
                seg.0 = self.tracker.downloaded.load(Ordering::Relaxed);
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), storm_core::StormError> {
        Write::flush(&mut self.file)?;
        Ok(())
    }
}
