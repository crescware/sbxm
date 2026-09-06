use super::GITHUB_TOKEN_ENV;

/// Sandbox内のgitへ設定する、期待するcredential helperの値。
///
/// helperはSandboxの環境変数を読むだけで、tokenの値そのものは持たない。gitはこれを
/// Basic認証として送り、proxyがgithub.com宛のrequest headerで本物のtokenへ差し替える。
pub(super) fn expected_credential_helper() -> String {
    format!("!f() {{ echo username=x; echo password=${GITHUB_TOKEN_ENV}; }}; f")
}
