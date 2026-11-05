/// SHA-1またはSHA-256の完全なGit object IDか。
pub fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
