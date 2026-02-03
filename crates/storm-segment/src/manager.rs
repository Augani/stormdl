use crate::splitter::{initial_segments, split_range};
use parking_lot::RwLock;
use std::sync::Arc;
use stormdl_core::{ByteRange, SegmentState, SegmentStatus};

pub struct SegmentManager {
    segments: Arc<RwLock<Vec<SegmentState>>>,
    total_size: u64,
    min_segment_size: u64,
    max_segments: usize,
}

impl SegmentManager {
    pub fn new(total_size: u64) -> Self {
        Self {
            segments: Arc::new(RwLock::new(Vec::new())),
            total_size,
            min_segment_size: 256 * 1024,
            max_segments: 32,
        }
    }

    pub fn with_config(total_size: u64, min_segment_size: u64, max_segments: usize) -> Self {
        Self {
            segments: Arc::new(RwLock::new(Vec::new())),
            total_size,
            min_segment_size,
            max_segments,
        }
    }

    pub fn with_segments(total_size: u64, num_segments: usize) -> Self {
        let manager = Self::new(total_size);
        let ranges = split_range(total_size, num_segments);
        let segments: Vec<SegmentState> = ranges
            .into_iter()
            .enumerate()
            .map(|(id, range)| SegmentState::new(id, range))
            .collect();
        *manager.segments.write() = segments;
        manager
    }

    pub fn initialize(&self) -> Vec<SegmentState> {
        let num_segments = initial_segments(self.total_size);
        let ranges = split_range(self.total_size, num_segments);

        let segments: Vec<SegmentState> = ranges
            .into_iter()
            .enumerate()
            .map(|(id, range)| SegmentState::new(id, range))
            .collect();

        *self.segments.write() = segments.clone();
        segments
    }

    pub fn get_segments(&self) -> Vec<SegmentState> {
        self.segments.read().clone()
    }

    pub fn update_segment(&self, id: usize, downloaded: u64, speed: f64) {
        let mut segments = self.segments.write();
        if let Some(segment) = segments.get_mut(id) {
            segment.downloaded = downloaded;
            segment.speed = speed;
        }
    }

    pub fn mark_complete(&self, id: usize) {
        let mut segments = self.segments.write();
        if let Some(segment) = segments.get_mut(id) {
            segment.status = SegmentStatus::Complete;
            segment.downloaded = segment.range.len();
        }
    }

    pub fn mark_error(&self, id: usize) {
        let mut segments = self.segments.write();
        if let Some(segment) = segments.get_mut(id) {
            segment.status = SegmentStatus::Error;
        }
    }

    pub fn split_segment(&self, id: usize) -> Option<SegmentState> {
        let mut segments = self.segments.write();

        if segments.len() >= self.max_segments {
            return None;
        }

        let segment = segments.get(id)?;
        let remaining = segment.remaining();

        if remaining < self.min_segment_size * 2 {
            return None;
        }

        let current_offset = segment.range.start + segment.downloaded;
        let split_point = current_offset + remaining / 2;

        let new_id = segments.len();
        let new_range = ByteRange::new(split_point, segment.range.end);

        segments.get_mut(id)?.range.end = split_point;

        let new_segment = SegmentState::new(new_id, new_range);
        segments.push(new_segment.clone());

        Some(new_segment)
    }

    pub fn total_downloaded(&self) -> u64 {
        self.segments.read().iter().map(|s| s.downloaded).sum()
    }

    pub fn all_complete(&self) -> bool {
        self.segments
            .read()
            .iter()
            .all(|s| s.status == SegmentStatus::Complete)
    }

    pub fn average_speed(&self) -> f64 {
        let segments = self.segments.read();
        let active: Vec<_> = segments
            .iter()
            .filter(|s| s.status == SegmentStatus::Active)
            .collect();

        if active.is_empty() {
            return 0.0;
        }

        active.iter().map(|s| s.speed).sum::<f64>() / active.len() as f64
    }
}
