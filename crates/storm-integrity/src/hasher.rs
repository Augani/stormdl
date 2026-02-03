use blake3::Hasher;

pub struct IncrementalHasher {
    hasher: Hasher,
    bytes_hashed: u64,
}

impl IncrementalHasher {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
            bytes_hashed: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_hashed += data.len() as u64;
    }

    pub fn finalize(&self) -> String {
        self.hasher.finalize().to_hex().to_string()
    }

    pub fn finalize_reset(&mut self) -> String {
        let hash = self.finalize();
        self.hasher.reset();
        self.bytes_hashed = 0;
        hash
    }

    pub fn bytes_hashed(&self) -> u64 {
        self.bytes_hashed
    }

    pub fn reset(&mut self) {
        self.hasher.reset();
        self.bytes_hashed = 0;
    }
}

impl Default for IncrementalHasher {
    fn default() -> Self {
        Self::new()
    }
}

pub fn hash_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_hasher() {
        let mut hasher = IncrementalHasher::new();
        hasher.update(b"hello ");
        hasher.update(b"world");

        let incremental_hash = hasher.finalize();
        let direct_hash = hash_bytes(b"hello world");

        assert_eq!(incremental_hash, direct_hash);
    }

    #[test]
    fn test_reset() {
        let mut hasher = IncrementalHasher::new();
        hasher.update(b"test");
        let hash1 = hasher.finalize_reset();

        hasher.update(b"test");
        let hash2 = hasher.finalize();

        assert_eq!(hash1, hash2);
    }
}
