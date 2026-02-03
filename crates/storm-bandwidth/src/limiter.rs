use governor::{
    Quota, RateLimiter as GovLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use std::num::NonZeroU32;
use std::sync::Arc;

type InnerLimiter = GovLimiter<NotKeyed, InMemoryState, DefaultClock>;

pub struct RateLimiter {
    limiter: Option<Arc<InnerLimiter>>,
    bytes_per_second: Option<u64>,
}

impl RateLimiter {
    pub fn new(bytes_per_second: Option<u64>) -> Self {
        let limiter = bytes_per_second.and_then(|bps| {
            if bps == 0 {
                return None;
            }
            let chunk_size = 16384u32;
            let chunks_per_second = (bps / chunk_size as u64).max(1) as u32;
            NonZeroU32::new(chunks_per_second)
                .map(|rate| Arc::new(GovLimiter::direct(Quota::per_second(rate))))
        });

        Self {
            limiter,
            bytes_per_second,
        }
    }

    pub fn unlimited() -> Self {
        Self {
            limiter: None,
            bytes_per_second: None,
        }
    }

    pub async fn acquire(&self, bytes: usize) {
        if let Some(ref limiter) = self.limiter {
            let chunks = (bytes / 16384).max(1);
            for _ in 0..chunks {
                limiter.until_ready().await;
            }
        }
    }

    pub fn try_acquire(&self, bytes: usize) -> bool {
        match &self.limiter {
            Some(limiter) => {
                let chunks = (bytes / 16384).max(1) as u32;
                if let Some(n) = NonZeroU32::new(chunks) {
                    limiter.check_n(n).is_ok()
                } else {
                    true
                }
            }
            None => true,
        }
    }

    pub fn is_limited(&self) -> bool {
        self.limiter.is_some()
    }

    pub fn limit(&self) -> Option<u64> {
        self.bytes_per_second
    }

    pub fn set_limit(&mut self, bytes_per_second: Option<u64>) {
        *self = Self::new(bytes_per_second);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::unlimited()
    }
}
