use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use storm_core::{DownloadId, DownloadOptions, Priority};

#[derive(Debug, Clone)]
pub struct QueuedDownload {
    pub id: DownloadId,
    pub options: DownloadOptions,
    pub priority: Priority,
}

pub struct DownloadQueue {
    queue: Arc<Mutex<VecDeque<QueuedDownload>>>,
    max_concurrent: usize,
    active_count: Arc<Mutex<usize>>,
}

impl DownloadQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            max_concurrent,
            active_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn enqueue(&self, download: QueuedDownload) {
        let mut queue = self.queue.lock();
        let insert_pos = queue
            .iter()
            .position(|d| d.priority as u8 > download.priority as u8)
            .unwrap_or(queue.len());
        queue.insert(insert_pos, download);
    }

    pub fn dequeue(&self) -> Option<QueuedDownload> {
        let active = *self.active_count.lock();
        if active >= self.max_concurrent {
            return None;
        }

        let mut queue = self.queue.lock();
        let download = queue.pop_front()?;

        *self.active_count.lock() += 1;
        Some(download)
    }

    pub fn complete(&self, _id: DownloadId) {
        let mut active = self.active_count.lock();
        *active = active.saturating_sub(1);
    }

    pub fn cancel(&self, id: DownloadId) {
        let mut queue = self.queue.lock();
        queue.retain(|d| d.id != id);
    }

    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    pub fn active_count(&self) -> usize {
        *self.active_count.lock()
    }

    pub fn can_start(&self) -> bool {
        *self.active_count.lock() < self.max_concurrent
    }

    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent = max;
    }

    pub fn reorder(&self, id: DownloadId, new_priority: Priority) {
        let mut queue = self.queue.lock();
        if let Some(pos) = queue.iter().position(|d| d.id == id) {
            let mut download = queue.remove(pos).unwrap();
            download.priority = new_priority;

            let insert_pos = queue
                .iter()
                .position(|d| d.priority as u8 > new_priority as u8)
                .unwrap_or(queue.len());
            queue.insert(insert_pos, download);
        }
    }
}

impl Default for DownloadQueue {
    fn default() -> Self {
        Self::new(3)
    }
}
