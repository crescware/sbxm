use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::support::Observed;

use super::{credential_key, expected_credential_helper, observe_git_credential};

/// Sandbox内のgitに、placeholderをcredentialとして使わせる。
///
/// 書く前に必ず再観測する。既に期待どおりならmutationを起こさず、値が無ければ
/// 設定する。別の値が既にある場合は、別の利用者のSandboxである可能性を捨てきれない
/// ため上書きしない。観測できない場合も同様に拒否する。
pub fn configure_git_credential(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    match observe_git_credential(host, sandbox)? {
        Observed::Matching => Ok(()),
        Observed::Missing => {
            crate::support::sandbox::exec(
                host,
                sandbox,
                &[
                    "git",
                    "config",
                    "--global",
                    &credential_key(),
                    &expected_credential_helper(),
                ],
            )?
            .require_success()?;
            Ok(())
        }
        Observed::Mismatch { evidence } => Err(refused(sandbox, &evidence)),
        Observed::Unobservable { evidence } => Err(unobservable(sandbox, &evidence)),
    }
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

fn unobservable(sandbox: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxCredentialHelperUnusable,
            msg!(
                "error-sandbox-credential-helper-unusable",
                sandbox = sandbox
            ),
        )
        .fact(Fact::cause(detail)),
    )
}
