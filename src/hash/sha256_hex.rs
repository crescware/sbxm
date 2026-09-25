use sha2::{Digest, Sha256};

use super::lowercase_hex;

/// SHA-256のlowercase hex。
pub fn sha256_hex(bytes: &[u8]) -> String {
    lowercase_hex(&Sha256::digest(bytes))
}
