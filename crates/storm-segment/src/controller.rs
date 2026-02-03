use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub struct AdaptiveController {
    current_segments: AtomicUsize,
    max_segments: usize,
    min_segment_size: u64,
    file_size: u64,
    last_adjustment: parking_lot::Mutex<Instant>,
    adjustment_interval: Duration,
}

impl AdaptiveController {
    pub fn new(file_size: u64, initial_segments: usize) -> Self {
        Self {
            current_segments: AtomicUsize::new(initial_segments),
            max_segments: 32,
            min_segment_size: 256 * 1024,
            file_size,
            last_adjustment: parking_lot::Mutex::new(Instant::now()),
            adjustment_interval: Duration::from_millis(500),
        }
    }

    pub fn with_config(
        file_size: u64,
        initial_segments: usize,
        max_segments: usize,
        min_segment_size: u64,
    ) -> Self {
        Self {
            current_segments: AtomicUsize::new(initial_segments),
            max_segments,
            min_segment_size,
            file_size,
            last_adjustment: parking_lot::Mutex::new(Instant::now()),
            adjustment_interval: Duration::from_millis(500),
        }
    }

    pub fn current_segments(&self) -> usize {
        self.current_segments.load(Ordering::Relaxed)
    }

    pub fn evaluate(&self, bdp: Option<u64>, _current_speed: f64) -> Option<SegmentAdjustment> {
        let mut last = self.last_adjustment.lock();
        if last.elapsed() < self.adjustment_interval {
            return None;
        }

        let current = self.current_segments.load(Ordering::Relaxed);
        let bdp = bdp?;

        let tcp_window = 65536u64;
        let optimal = ((bdp as f64) / (tcp_window as f64)).ceil() as usize;
        let optimal = optimal.clamp(1, self.max_segments);

        if optimal <= current {
            return None;
        }

        let remaining_segments = optimal - current;
        let segments_to_add = remaining_segments.min(4);

        let avg_segment_size = self.file_size / (current + segments_to_add) as u64;
        if avg_segment_size < self.min_segment_size {
            return None;
        }

        *last = Instant::now();
        self.current_segments
            .store(current + segments_to_add, Ordering::Relaxed);

        Some(SegmentAdjustment::Split {
            count: segments_to_add,
            reason: AdjustmentReason::BdpIncrease { bdp, optimal },
        })
    }

    pub fn should_split_slow_segment(
        &self,
        segment_speed: f64,
        avg_speed: f64,
        remaining_bytes: u64,
    ) -> bool {
        if remaining_bytes < self.min_segment_size * 2 {
            return false;
        }

        let current = self.current_segments.load(Ordering::Relaxed);
        if current >= self.max_segments {
            return false;
        }

        let threshold = avg_speed * 0.3;
        segment_speed > 0.0 && segment_speed < threshold
    }

    pub fn record_split(&self) {
        self.current_segments.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub enum SegmentAdjustment {
    Split {
        count: usize,
        reason: AdjustmentReason,
    },
    #[allow(dead_code)]
    Merge {
        count: usize,
        reason: AdjustmentReason,
    },
}

#[derive(Debug, Clone)]
pub enum AdjustmentReason {
    BdpIncrease {
        bdp: u64,
        optimal: usize,
    },
    #[allow(dead_code)]
    SlowSegment {
        speed: f64,
        threshold: f64,
    },
    #[allow(dead_code)]
    ConnectionLimit,
}
