use super::is_login_unavailable;

/// Dockerのlogin serviceが、host復帰直後の認証更新の抑止期間を明示した失敗。
///
/// sbx v0.51.0（`7d55286`）の`sandboxlib/auth/wake_state.go`は、復帰後120秒間の更新を抑止する。
/// 通常の一時障害からは推測せず、この期間に入ったと明示された場合だけを認識する。
pub fn is_login_resume_holdoff(stderr: &[u8]) -> bool {
    is_login_unavailable(stderr)
        && String::from_utf8_lossy(stderr)
            .to_ascii_lowercase()
            .contains("within the post-resume hold-off window")
}
