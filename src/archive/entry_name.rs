use super::{BLOCK, trimmed};

/// ustar headerのnameとprefixからentry名を組み立てる。
pub(super) fn entry_name(header: &[u8; BLOCK]) -> Option<String> {
    let name = trimmed(&header[0..100])?;
    let prefix = trimmed(&header[345..500])?;
    if prefix.is_empty() {
        Some(name)
    } else {
        Some(format!("{prefix}/{name}"))
    }
}
