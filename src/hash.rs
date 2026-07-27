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
mod tests {
    use super::*;

    #[test]
    fn known_inputs_produce_the_published_digest() {
        assert_eq!(
            sha256_hex(b"owner/repository"),
            "21da1b566c9f5080441b32f57a37f33e921c5d8be8b686584f9ce819d66732fe"
        );
        assert_eq!(
            sha256_hex(b"example-org/example-repo"),
            "99a40327a69bb5dc074f625c045448b0f3fe0587155e74276758abce34759ead"
        );
    }

    #[test]
    fn digests_are_lowercase_hex_of_a_fixed_length() {
        let digest = sha256_hex(b"a/b");
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }

    #[test]
    fn the_short_form_keeps_the_leading_twelve_digits() {
        let full = sha256_hex(b"a/b");
        assert_eq!(short_hex(&full), "c14cddc033f6");
        assert_eq!(short_hex(&full).len(), SHORT_HEX_LENGTH);
        assert_eq!(short_hex("abc"), "abc");
    }
}
