/// group・otherに権限が残っているか。
pub fn permission_too_open(mode: u32) -> bool {
    mode & 0o077 != 0
}
