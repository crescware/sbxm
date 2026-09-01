use super::GITHUB_HOST;

/// `Sandbox内のgitがGitHubへ提示するcredentialのconfig` key。
///
/// github.comだけに絞る。ほかのhostへplaceholderを送っても意味がなく、送る相手を
/// 広げる理由がない。
pub(super) fn credential_key() -> String {
    format!("credential.https://{GITHUB_HOST}.helper")
}
