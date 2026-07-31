use super::trimmed;

pub(super) fn octal(field: &[u8]) -> Option<u64> {
    let text = trimmed(field)?;
    if text.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(&text, 8).ok()
}
