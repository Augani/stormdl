use parking_lot::Mutex;
use std::time::{Duration, Instant};

const SAMPLE_WINDOW: usize = 10;
const RTT_SAMPLE_WINDOW: usize = 20;
const EWMA_ALPHA: f64 = 0.2;

pub struct NetworkMonitor {
    samples: Mutex<Vec<(Instant, u64)>>,
    last_bytes: Mutex<u64>,
    rtt_samples: Mutex<Vec<Duration>>,
    smoothed_rtt: Mutex<Option<f64>>,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            samples: Mutex::new(Vec::with_capacity(SAMPLE_WINDOW)),
            last_bytes: Mutex::new(0),
            rtt_samples: Mutex::new(Vec::with_capacity(RTT_SAMPLE_WINDOW)),
            smoothed_rtt: Mutex::new(None),
        }
    }

    pub fn record(&self, bytes: u64) {
        let now = Instant::now();
        let mut samples = self.samples.lock();

        samples.push((now, bytes));

        let cutoff = now - Duration::from_secs(10);
        samples.retain(|(t, _)| *t > cutoff);

        if samples.len() > SAMPLE_WINDOW {
            samples.remove(0);
        }

        *self.last_bytes.lock() = bytes;
    }

    pub fn record_rtt(&self, rtt: Duration) {
        let rtt_ms = rtt.as_secs_f64() * 1000.0;

        let mut samples = self.rtt_samples.lock();
        samples.push(rtt);
        if samples.len() > RTT_SAMPLE_WINDOW {
            samples.remove(0);
        }

        let mut smoothed = self.smoothed_rtt.lock();
        *smoothed = Some(match *smoothed {
            Some(prev) => prev * (1.0 - EWMA_ALPHA) + rtt_ms * EWMA_ALPHA,
            None => rtt_ms,
        });
    }

    pub fn smoothed_rtt(&self) -> Option<Duration> {
        self.smoothed_rtt
            .lock()
            .map(|ms| Duration::from_secs_f64(ms / 1000.0))
    }

    pub fn min_rtt(&self) -> Option<Duration> {
        self.rtt_samples.lock().iter().min().copied()
    }

    pub fn current_speed(&self) -> f64 {
        let samples = self.samples.lock();
        if samples.len() < 2 {
            return 0.0;
        }

        let (first_time, first_bytes) = samples.first().unwrap();
        let (last_time, last_bytes) = samples.last().unwrap();

        let elapsed = last_time.duration_since(*first_time).as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }

        (last_bytes - first_bytes) as f64 / elapsed
    }

    pub fn average_speed(&self) -> f64 {
        let samples = self.samples.lock();
        if samples.len() < 2 {
            return 0.0;
        }

        let total_bytes: u64 = samples.iter().map(|(_, b)| b).sum();
        let elapsed = samples
            .last()
            .unwrap()
            .0
            .duration_since(samples.first().unwrap().0)
            .as_secs_f64();

        if elapsed < 0.001 {
            return 0.0;
        }

        total_bytes as f64 / elapsed / samples.len() as f64
    }

    pub fn bandwidth_delay_product(&self) -> Option<u64> {
        let speed = self.current_speed();
        if speed <= 0.0 {
            return None;
        }

        self.smoothed_rtt()
            .map(|rtt| (speed * rtt.as_secs_f64()) as u64)
    }

    pub fn optimal_segment_count(&self, file_size: u64) -> Option<usize> {
        let bdp = self.bandwidth_delay_product()?;
        if bdp == 0 {
            return None;
        }

        let tcp_window = 65536u64;
        let min_connections = ((bdp as f64) / (tcp_window as f64)).ceil() as usize;

        let max_connections = match file_size {
            0..=1_000_000 => 1,
            1_000_001..=10_000_000 => 4,
            10_000_001..=100_000_000 => 8,
            100_000_001..=1_000_000_000 => 16,
            _ => 32,
        };

        Some(min_connections.clamp(1, max_connections))
    }

    pub fn reset(&self) {
        self.samples.lock().clear();
        *self.last_bytes.lock() = 0;
        self.rtt_samples.lock().clear();
        *self.smoothed_rtt.lock() = None;
    }
}

impl Default for NetworkMonitor {
    fn default() -> Self {
        Self::new()
    }
}
