//! 決定的なcontent hash。
//!
//! Sandbox名とimage世代は同じhash関数から導出する。hexはlowercaseで統一する。

use sha2::{Digest, Sha256};

/// 世代を表す短縮hexの桁数。
pub const SHORT_HEX_LENGTH: usize = 12;

/// SHA-256のlowercase hex。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 世代の識別に使う先頭12桁。
pub fn short_hex(full: &str) -> &str {
    &full[..SHORT_HEX_LENGTH.min(full.len())]
}

#[cfg(test)]
#[path = "hash_test.rs"]
mod hash_test;
