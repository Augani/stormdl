use crate::SegmentManager;
use std::sync::Arc;
use stormdl_core::SegmentStatus;

pub struct Rebalancer {
    manager: Arc<SegmentManager>,
    slow_threshold_pct: f64,
    min_segment_size: u64,
    max_segments: usize,
}

impl Rebalancer {
    pub fn new(manager: Arc<SegmentManager>) -> Self {
        Self {
            manager,
            slow_threshold_pct: 0.2,
            min_segment_size: 256 * 1024,
            max_segments: 32,
        }
    }

    pub fn with_threshold(manager: Arc<SegmentManager>, slow_threshold_pct: f64) -> Self {
        Self {
            manager,
            slow_threshold_pct,
            min_segment_size: 256 * 1024,
            max_segments: 32,
        }
    }

    pub fn with_config(
        manager: Arc<SegmentManager>,
        slow_threshold_pct: f64,
        min_segment_size: u64,
        max_segments: usize,
    ) -> Self {
        Self {
            manager,
            slow_threshold_pct,
            min_segment_size,
            max_segments,
        }
    }

    pub fn check_and_rebalance(&self) -> Vec<usize> {
        self.check_and_rebalance_with_bdp(None)
    }

    pub fn check_and_rebalance_with_bdp(&self, bdp: Option<u64>) -> Vec<usize> {
        let segments = self.manager.get_segments();
        let avg_speed = self.manager.average_speed();

        if avg_speed <= 0.0 {
            return vec![];
        }

        let current_count = segments.len();
        if current_count >= self.max_segments {
            return vec![];
        }

        let slow_threshold = self.calculate_threshold(avg_speed, bdp);
        let mut new_segments = Vec::new();

        for segment in &segments {
            if segment.status != SegmentStatus::Active {
                continue;
            }

            let remaining = segment.remaining();
            if remaining < self.min_segment_size * 2 {
                continue;
            }

            if segment.speed < slow_threshold && remaining > 0 {
                if current_count + new_segments.len() >= self.max_segments {
                    break;
                }

                if let Some(new_seg) = self.manager.split_segment(segment.id) {
                    tracing::info!(
                        "Split slow segment {} (speed: {:.2} KB/s, avg: {:.2} KB/s, threshold: {:.2} KB/s) -> new segment {}",
                        segment.id,
                        segment.speed / 1024.0,
                        avg_speed / 1024.0,
                        slow_threshold / 1024.0,
                        new_seg.id
                    );
                    new_segments.push(new_seg.id);
                }
            }
        }

        new_segments
    }

    fn calculate_threshold(&self, avg_speed: f64, bdp: Option<u64>) -> f64 {
        let base_threshold = avg_speed * self.slow_threshold_pct;

        if let Some(bdp) = bdp {
            let tcp_window = 65536.0;
            let bdp_factor = (bdp as f64 / tcp_window).sqrt();
            let adjusted_pct = (self.slow_threshold_pct * bdp_factor).min(0.5);
            avg_speed * adjusted_pct
        } else {
            base_threshold
        }
    }

    pub fn optimal_segments_from_bdp(&self, bdp: u64, file_size: u64) -> usize {
        let tcp_window = 65536u64;
        let min_connections = ((bdp as f64) / (tcp_window as f64)).ceil() as usize;

        let size_based_max = match file_size {
            0..=1_000_000 => 1,
            1_000_001..=10_000_000 => 4,
            10_000_001..=100_000_000 => 8,
            100_000_001..=1_000_000_000 => 16,
            _ => 32,
        };

        min_connections.clamp(1, size_based_max.min(self.max_segments))
    }
}
