use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{SecretListing, is_global_scope};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{
    GITHUB_SERVICE, GITHUB_TOKEN_ENV, forget_custom_command, list_secrets, register_command,
};

/// `GitHubのservice` secretが登録済みであることを確認する。
///
/// 未登録なら、発行条件と登録commandを示して前提条件不足として停止する。Sandbox限定の
/// service secretは登録した時点で効くが、Sandboxを作る前に確認しておくと、tokenの
/// ないままimageのbuildやTemplateのloadへ進まずに済む。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let listing = list_secrets(host)?;
    if covered(&listing, sandbox) {
        return Ok(());
    }

    let mut diagnostic = Diagnostic::new(
        ErrorId::GithubSecretMissing,
        msg!(
            "error-github-secret-missing",
            sandbox = sandbox,
            service = GITHUB_SERVICE
        ),
    );
    let mut remediation = Remediation::text(msg!("remediation-github-secret-missing"));

    // 以前の版が案内した`--env GH_TOKEN`のcustom secretは、組み込みserviceに影に
    // されて届かない。登録してあるのに動かない理由をここで示し、消す手順も添える。
    let shadowed: Vec<_> = listing
        .customs
        .iter()
        .filter(|custom| custom.env == GITHUB_TOKEN_ENV)
        .filter(|custom| custom.scope == sandbox || is_global_scope(&custom.scope))
        .collect();
    if !shadowed.is_empty() {
        diagnostic = diagnostic.fact(Fact::reason(msg!(
            "cause-github-custom-secret-shadowed",
            env = GITHUB_TOKEN_ENV,
            service = GITHUB_SERVICE
        )));
        remediation = remediation.explain(msg!("remediation-github-custom-secret-shadowed"));
        for custom in shadowed.iter().filter(|custom| custom.scope == sandbox) {
            remediation = remediation.try_run(forget_custom_command(sandbox, &custom.placeholder));
        }
    }

    Err(Error::single(diagnostic.remediation(
        remediation.try_run(register_command(sandbox)),
    )))
}

/// このSandboxが`github` serviceのtokenを受けられるか。
///
/// Sandbox限定の登録か、global scopeの登録のどちらかがあればよい。
fn covered(listing: &SecretListing, sandbox: &str) -> bool {
    listing.services.iter().any(|service| {
        service.name == GITHUB_SERVICE
            && (service.scope == sandbox || is_global_scope(&service.scope))
    })
}
