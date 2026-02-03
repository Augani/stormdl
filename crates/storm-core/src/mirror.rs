use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MirrorPriority {
    Primary,
    #[default]
    Secondary,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mirror {
    pub url: Url,
    pub priority: MirrorPriority,
    pub region: Option<String>,
    pub max_connections: Option<usize>,
}

impl Mirror {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            priority: MirrorPriority::Secondary,
            region: None,
            max_connections: None,
        }
    }

    pub fn primary(url: Url) -> Self {
        Self {
            url,
            priority: MirrorPriority::Primary,
            region: None,
            max_connections: None,
        }
    }

    pub fn with_priority(mut self, priority: MirrorPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = Some(max);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct MirrorStats {
    pub bytes_downloaded: u64,
    pub errors: usize,
    pub avg_speed: f64,
    pub active_segments: usize,
}

#[derive(Debug, Clone)]
pub struct MirrorSet {
    mirrors: Vec<Mirror>,
    stats: HashMap<usize, MirrorStats>,
}

impl MirrorSet {
    pub fn new(primary: Url) -> Self {
        Self {
            mirrors: vec![Mirror::primary(primary)],
            stats: HashMap::new(),
        }
    }

    pub fn add(&mut self, mirror: Mirror) {
        self.mirrors.push(mirror);
    }

    pub fn add_url(&mut self, url: Url) {
        self.mirrors.push(Mirror::new(url));
    }

    pub fn mirrors(&self) -> &[Mirror] {
        &self.mirrors
    }

    pub fn len(&self) -> usize {
        self.mirrors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mirrors.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Mirror> {
        self.mirrors.get(index)
    }

    pub fn update_stats(&mut self, index: usize, stats: MirrorStats) {
        self.stats.insert(index, stats);
    }

    pub fn get_stats(&self, index: usize) -> Option<&MirrorStats> {
        self.stats.get(&index)
    }

    pub fn best_mirror(&self) -> usize {
        if self.mirrors.len() <= 1 {
            return 0;
        }

        let mut best_idx = 0;
        let mut best_score = f64::NEG_INFINITY;

        for (idx, mirror) in self.mirrors.iter().enumerate() {
            let stats = self.stats.get(&idx);
            let speed = stats.map(|s| s.avg_speed).unwrap_or(0.0);
            let errors = stats.map(|s| s.errors).unwrap_or(0);
            let active = stats.map(|s| s.active_segments).unwrap_or(0);

            let priority_boost = match mirror.priority {
                MirrorPriority::Primary => 1.5,
                MirrorPriority::Secondary => 1.0,
                MirrorPriority::Fallback => 0.5,
            };

            let error_penalty = 1.0 / (1.0 + errors as f64 * 0.5);
            let load_factor = 1.0 / (1.0 + active as f64 * 0.1);

            let score = speed * priority_boost * error_penalty * load_factor;

            if score > best_score {
                best_score = score;
                best_idx = idx;
            }
        }

        best_idx
    }

    pub fn select_for_segment(&self, _segment_idx: usize) -> usize {
        self.best_mirror()
    }
}

impl From<Url> for MirrorSet {
    fn from(url: Url) -> Self {
        Self::new(url)
    }
}

impl From<Vec<Url>> for MirrorSet {
    fn from(urls: Vec<Url>) -> Self {
        if urls.is_empty() {
            panic!("MirrorSet requires at least one URL");
        }

        let mut set = Self::new(urls[0].clone());
        for url in urls.into_iter().skip(1) {
            set.add_url(url);
        }
        set
    }
}
