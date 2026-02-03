use crate::hasher::hash_bytes;
use storm_core::StormError;
use std::path::Path;

pub struct ContentVerifier {
    expected_hash: String,
    algorithm: HashAlgorithm,
}

#[derive(Debug, Clone, Copy)]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
    Md5,
}

impl ContentVerifier {
    pub fn new(expected_hash: String, algorithm: HashAlgorithm) -> Self {
        Self {
            expected_hash,
            algorithm,
        }
    }

    pub fn verify(&self, data: &[u8]) -> Result<(), StormError> {
        let actual_hash = match self.algorithm {
            HashAlgorithm::Blake3 => hash_bytes(data),
            HashAlgorithm::Sha256 | HashAlgorithm::Md5 => {
                return Err(StormError::Other(format!(
                    "{:?} verification not yet implemented",
                    self.algorithm
                )));
            }
        };

        if actual_hash == self.expected_hash {
            Ok(())
        } else {
            Err(StormError::HashMismatch {
                expected: self.expected_hash.clone(),
                actual: actual_hash,
            })
        }
    }
}

pub fn verify_content(data: &[u8], expected_hash: &str) -> Result<(), StormError> {
    let actual_hash = hash_bytes(data);
    if actual_hash == expected_hash {
        Ok(())
    } else {
        Err(StormError::HashMismatch {
            expected: expected_hash.to_string(),
            actual: actual_hash,
        })
    }
}

pub async fn verify_file(path: &Path, expected_hash: &str) -> Result<(), StormError> {
    let data = tokio::fs::read(path).await?;
    verify_content(&data, expected_hash)
}
