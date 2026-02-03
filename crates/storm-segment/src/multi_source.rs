use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use stormdl_core::{ByteRange, MirrorSet, MirrorStats};

pub struct MultiSourceManager {
    mirrors: RwLock<MirrorSet>,
    segment_assignments: RwLock<HashMap<usize, usize>>,
    source_stats: RwLock<HashMap<usize, SourceStats>>,
    #[allow(dead_code)]
    total_size: u64,
}

struct SourceStats {
    bytes_downloaded: AtomicU64,
    errors: AtomicUsize,
    active_segments: AtomicUsize,
    speed_samples: RwLock<Vec<f64>>,
}

impl SourceStats {
    fn new() -> Self {
        Self {
            bytes_downloaded: AtomicU64::new(0),
            errors: AtomicUsize::new(0),
            active_segments: AtomicUsize::new(0),
            speed_samples: RwLock::new(Vec::with_capacity(10)),
        }
    }

    fn avg_speed(&self) -> f64 {
        let samples = self.speed_samples.read();
        if samples.is_empty() {
            return 0.0;
        }
        samples.iter().sum::<f64>() / samples.len() as f64
    }
}

impl MultiSourceManager {
    pub fn new(mirrors: MirrorSet, total_size: u64) -> Self {
        Self {
            mirrors: RwLock::new(mirrors),
            segment_assignments: RwLock::new(HashMap::new()),
            source_stats: RwLock::new(HashMap::new()),
            total_size,
        }
    }

    pub fn assign_segment(&self, segment_idx: usize, _range: ByteRange) -> usize {
        let mirrors = self.mirrors.read();
        let source_idx = mirrors.select_for_segment(segment_idx);
        drop(mirrors);

        self.segment_assignments
            .write()
            .insert(segment_idx, source_idx);

        let mut stats = self.source_stats.write();
        stats
            .entry(source_idx)
            .or_insert_with(SourceStats::new)
            .active_segments
            .fetch_add(1, Ordering::Relaxed);

        source_idx
    }

    pub fn get_assignment(&self, segment_idx: usize) -> Option<usize> {
        self.segment_assignments.read().get(&segment_idx).copied()
    }

    pub fn reassign_segment(&self, segment_idx: usize) -> Option<usize> {
        let old_source = {
            let assignments = self.segment_assignments.read();
            assignments.get(&segment_idx).copied()
        };

        if let Some(old_idx) = old_source {
            let stats = self.source_stats.read();
            if let Some(source_stats) = stats.get(&old_idx) {
                source_stats.active_segments.fetch_sub(1, Ordering::Relaxed);
            }
        }

        let mirrors = self.mirrors.read();
        let mirror_count = mirrors.len();

        if mirror_count <= 1 {
            return None;
        }

        let excluded = old_source.unwrap_or(usize::MAX);
        let mut best_idx = None;
        let mut best_score = f64::NEG_INFINITY;

        for idx in 0..mirror_count {
            if idx == excluded {
                continue;
            }

            let stats_guard = self.source_stats.read();
            let stats = stats_guard.get(&idx);

            let speed = stats.map(|s| s.avg_speed()).unwrap_or(0.0);
            let errors = stats.map(|s| s.errors.load(Ordering::Relaxed)).unwrap_or(0);
            let active = stats
                .map(|s| s.active_segments.load(Ordering::Relaxed))
                .unwrap_or(0);

            let error_penalty = 1.0 / (1.0 + errors as f64 * 0.5);
            let load_factor = 1.0 / (1.0 + active as f64 * 0.1);
            let score = (speed + 1.0) * error_penalty * load_factor;

            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(new_idx) = best_idx {
            self.segment_assignments
                .write()
                .insert(segment_idx, new_idx);

            let mut stats = self.source_stats.write();
            stats
                .entry(new_idx)
                .or_insert_with(SourceStats::new)
                .active_segments
                .fetch_add(1, Ordering::Relaxed);
        }

        best_idx
    }

    pub fn record_progress(&self, source_idx: usize, bytes: u64, speed: f64) {
        let mut stats = self.source_stats.write();
        let source_stats = stats.entry(source_idx).or_insert_with(SourceStats::new);

        source_stats
            .bytes_downloaded
            .fetch_add(bytes, Ordering::Relaxed);

        let mut samples = source_stats.speed_samples.write();
        samples.push(speed);
        if samples.len() > 10 {
            samples.remove(0);
        }
    }

    pub fn record_error(&self, source_idx: usize) {
        let mut stats = self.source_stats.write();
        stats
            .entry(source_idx)
            .or_insert_with(SourceStats::new)
            .errors
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn complete_segment(&self, segment_idx: usize) {
        let source_idx = {
            let assignments = self.segment_assignments.read();
            assignments.get(&segment_idx).copied()
        };

        if let Some(idx) = source_idx {
            let stats = self.source_stats.read();
            if let Some(source_stats) = stats.get(&idx) {
                source_stats.active_segments.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    pub fn get_mirror_url(&self, source_idx: usize) -> Option<url::Url> {
        self.mirrors.read().get(source_idx).map(|m| m.url.clone())
    }

    pub fn mirror_count(&self) -> usize {
        self.mirrors.read().len()
    }

    pub fn sync_mirror_stats(&self) {
        let stats_guard = self.source_stats.read();
        let mut mirrors = self.mirrors.write();

        for (idx, stats) in stats_guard.iter() {
            let mirror_stats = MirrorStats {
                bytes_downloaded: stats.bytes_downloaded.load(Ordering::Relaxed),
                errors: stats.errors.load(Ordering::Relaxed),
                avg_speed: stats.avg_speed(),
                active_segments: stats.active_segments.load(Ordering::Relaxed),
            };
            mirrors.update_stats(*idx, mirror_stats);
        }
    }

    pub fn get_source_summary(&self) -> Vec<(usize, u64, f64, usize)> {
        let stats = self.source_stats.read();
        stats
            .iter()
            .map(|(idx, s)| {
                (
                    *idx,
                    s.bytes_downloaded.load(Ordering::Relaxed),
                    s.avg_speed(),
                    s.errors.load(Ordering::Relaxed),
                )
            })
            .collect()
    }
}
