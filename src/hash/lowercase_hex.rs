use std::fmt::Write as _;

/// byte列のlowercase hex。
pub(super) fn lowercase_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Stringへの書き込みは失敗しない。
        let _ = write!(out, "{byte:02x}");
    }
    out
}
