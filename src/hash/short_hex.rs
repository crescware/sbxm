use super::SHORT_HEX_LENGTH;

/// 世代の識別に使う先頭12桁。
pub fn short_hex(full: &str) -> &str {
    &full[..SHORT_HEX_LENGTH.min(full.len())]
}
