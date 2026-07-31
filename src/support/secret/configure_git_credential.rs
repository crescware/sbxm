use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::{GITHUB_HOST, GITHUB_TOKEN_ENV};

/// `Sandbox内のgitがGitHubへ提示するcredentialのconfig` key。
///
/// github.comだけに絞る。ほかのhostへplaceholderを送っても意味がなく、送る相手を
/// 広げる理由がない。
fn credential_key() -> String {
    format!("credential.https://{GITHUB_HOST}.helper")
}

/// Sandbox内のgitに、placeholderをcredentialとして使わせる。
///
/// helperはplaceholderを読むだけで、tokenは持たない。gitはこれをBasic認証として
/// 送り、proxyがgithub.com宛のrequest headerで本物のtokenへ差し替える。usernameは
/// `GitHubのgit` endpointでは任意の値でよい。
pub fn configure_git_credential(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let helper = format!("!f() {{ echo username=x; echo password=${GITHUB_TOKEN_ENV}; }}; f");
    crate::support::sandbox::exec(
        host,
        sandbox,
        &["git", "config", "--global", &credential_key(), &helper],
    )?
    .require_success()?;
    Ok(())
}
