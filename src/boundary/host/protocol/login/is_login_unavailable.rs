/// 復帰後のsession更新などで、Dockerのlogin serviceが一時的に応答できない失敗。
///
/// 認証不在や、別のserviceの503を再試行対象へ広げない。
pub fn is_login_unavailable(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    text.contains("503 service unavailable") && text.contains("docker login service unavailable")
}
