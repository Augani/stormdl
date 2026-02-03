pub struct WriteBuffer {
    data: Vec<u8>,
    capacity: usize,
}

impl WriteBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn append(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn would_overflow(&self, additional: usize) -> bool {
        self.data.len() + additional > self.capacity
    }

    pub fn is_full(&self) -> bool {
        self.data.len() >= self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_buffer() {
        let mut buffer = WriteBuffer::new(100);
        assert!(buffer.is_empty());

        buffer.append(&[1, 2, 3]);
        assert_eq!(buffer.len(), 3);
        assert!(!buffer.is_full());

        assert!(buffer.would_overflow(98));
        assert!(!buffer.would_overflow(97));

        buffer.clear();
        assert!(buffer.is_empty());
    }
}
