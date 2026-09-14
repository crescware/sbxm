use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::support::Observed;

use super::{credential_key, expected_credential_helper, is_sbxm_helper, observe_git_credential};

/// Sandbox内のgitに、登録済みのplaceholderをcredentialとして使わせる。
///
/// 書く前に必ず再観測する。既に期待どおりならmutationを起こさない。値が無い場合と、
/// sbxm自身が書いた値が残っている場合は書く。後者は、tokenを登録し直してplaceholderが
/// 変わった場合と、環境変数を読む旧版の形が残っている場合を含む。どちらも次のfetchが
/// 認証できなくなるため、観測したその場で現在の値へ揃える。
///
/// 形の違う値が既にある場合は、別の利用者のSandboxである可能性を捨てきれないため
/// 上書きしない。観測できない場合も同様に拒否する。
pub fn configure_git_credential(
    host: &dyn HostEnvironment,
    sandbox: &str,
    placeholder: &str,
) -> Result<()> {
    match observe_git_credential(host, sandbox, placeholder)? {
        Observed::Matching => Ok(()),
        Observed::Missing => write(host, sandbox, placeholder),
        Observed::Mismatch { evidence } if is_sbxm_helper(&evidence) => {
            write(host, sandbox, placeholder)
        }
        Observed::Mismatch { evidence } => Err(refused(sandbox, &evidence)),
        Observed::Unobservable { evidence } => Err(unobservable(sandbox, &evidence)),
    }
}

fn write(host: &dyn HostEnvironment, sandbox: &str, placeholder: &str) -> Result<()> {
    crate::support::sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "config",
            "--global",
            &credential_key(),
            &expected_credential_helper(placeholder),
        ],
    )?
    .require_success()?;
    Ok(())
}

fn refused(sandbox: &str, observed: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxCredentialHelperUnusable,
            msg!(
                "error-sandbox-credential-helper-unusable",
                sandbox = sandbox
            ),
        )
        .fact(Fact::reason(msg!(
            "cause-credential-helper-differs",
            observed = observed
        ))),
    )
}

fn unobservable(sandbox: &str, evidence: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxCredentialHelperUnusable,
            msg!(
                "error-sandbox-credential-helper-unusable",
                sandbox = sandbox
            ),
        )
        .fact(Fact::cause(evidence)),
    )
}
