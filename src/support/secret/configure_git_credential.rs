use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

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
    let key = credential_key();
    let helper = helper();
    let observed = crate::support::sandbox::exec(
        host,
        sandbox,
        &["git", "config", "--global", "--get", &key],
    )?;
    if observed.success() {
        let value = observed.stdout_text().trim().to_string();
        if value == helper {
            return Ok(());
        }
        if !value.is_empty() {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxIdentityMismatch,
                    msg!(
                        "error-sandbox-identity-mismatch",
                        sandbox = sandbox,
                        key = key,
                        observed = value,
                        expected = helper
                    ),
                )
                .fact(Fact::value(&key))
                .remediation(msg!("remediation-sandbox-identity-mismatch")),
            ));
        }
    }
    crate::support::sandbox::exec(host, sandbox, &["git", "config", "--global", &key, &helper])?
        .require_success()?;
    Ok(())
}

fn helper() -> String {
    format!("!f() {{ echo username=x; echo password=${GITHUB_TOKEN_ENV}; }}; f")
}
