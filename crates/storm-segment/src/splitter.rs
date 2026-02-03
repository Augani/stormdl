use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum SplitStrategy {
    Static(usize),
    Adaptive,
}

pub fn optimal_segments(file_size: u64, measured_bw: f64, rtt: Duration) -> usize {
    let bdp = measured_bw * rtt.as_secs_f64();
    let tcp_window = 65536.0;
    let min_connections = (bdp / tcp_window).ceil() as usize;

    let max_connections = match file_size {
        0..=1_000_000 => 1,
        1_000_001..=10_000_000 => 4,
        10_000_001..=100_000_000 => 8,
        100_000_001..=1_000_000_000 => 16,
        _ => 32,
    };

    min_connections.clamp(1, max_connections)
}

pub fn initial_segments(file_size: u64) -> usize {
    match file_size {
        0..=1_000_000 => 1,
        1_000_001..=10_000_000 => 4,
        _ => 8,
    }
}

pub fn turbo_segments(file_size: u64) -> usize {
    match file_size {
        0..=1_000_000 => 1,
        1_000_001..=10_000_000 => 8,
        _ => 16,
    }
}

pub fn split_range(total_size: u64, num_segments: usize) -> Vec<storm_core::ByteRange> {
    if num_segments == 0 || total_size == 0 {
        return vec![];
    }

    let segment_size = total_size / num_segments as u64;
    let remainder = total_size % num_segments as u64;

    let mut ranges = Vec::with_capacity(num_segments);
    let mut offset = 0;

    for i in 0..num_segments {
        let extra = if i < remainder as usize { 1 } else { 0 };
        let size = segment_size + extra;
        ranges.push(storm_core::ByteRange::new(offset, offset + size));
        offset += size;
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_range_even() {
        let ranges = split_range(100, 4);
        assert_eq!(ranges.len(), 4);
        assert_eq!(ranges[0], storm_core::ByteRange::new(0, 25));
        assert_eq!(ranges[1], storm_core::ByteRange::new(25, 50));
        assert_eq!(ranges[2], storm_core::ByteRange::new(50, 75));
        assert_eq!(ranges[3], storm_core::ByteRange::new(75, 100));
    }

    #[test]
    fn test_split_range_uneven() {
        let ranges = split_range(10, 3);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].len() + ranges[1].len() + ranges[2].len(), 10);
    }
}
