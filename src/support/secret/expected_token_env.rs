use super::{GITHUB_TOKEN_ENV, GITHUB_TOKEN_FALLBACK_ENV, TOKEN_ENV_MARKER};

/// token環境変数fileの、期待する中身。
///
/// 置くのはplaceholderであり、tokenではない。Sandboxの中のprocessがこれをGitHubへ
/// 送ると、proxyが登録済みhost宛のrequestで本物のtokenへ差し替える。`gh`は
/// `GH_TOKEN`を先に読み、無ければ`GITHUB_TOKEN`を読む。組み込みserviceは両方を
/// sentinelで埋めるため、両方を上書きしないと片方だけが残る。
pub(super) fn expected_token_env(placeholder: &str) -> String {
    format!(
        "{TOKEN_ENV_MARKER} The placeholder is substituted by the Docker Sandboxes proxy.\n\
         export {GITHUB_TOKEN_ENV}={placeholder}\n\
         export {GITHUB_TOKEN_FALLBACK_ENV}={placeholder}\n"
    )
}
