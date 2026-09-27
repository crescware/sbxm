use super::token_env_lines;

/// token環境変数fileの、期待する中身。
///
/// 置くのはplaceholderであり、tokenではない。Sandboxの中のprocessがこれをGitHubへ
/// 送ると、proxyが登録済みhost宛のrequestで本物のtokenへ差し替える。`gh`は
/// `GH_TOKEN`を先に読み、無ければ`GITHUB_TOKEN`を読む。組み込みserviceは両方を
/// sentinelで埋めるため、両方を上書きしないと片方だけが残る。
pub(super) fn expected_token_env(placeholder: &str) -> String {
    let mut content = token_env_lines(placeholder).join("\n");
    content.push('\n');
    content
}
