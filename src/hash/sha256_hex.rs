use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// SHA-256のlowercase hex。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Stringへの書き込みは失敗しない。
        let _ = write!(out, "{byte:02x}");
    }
    out
}
