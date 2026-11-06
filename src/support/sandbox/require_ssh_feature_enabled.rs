use crate::boundary::host::protocol::SshFeature;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// 同じ観測の有効値を、追加の外部commandなしで判定する。
pub(crate) fn require_ssh_feature_enabled(feature: &SshFeature) -> Result<()> {
    if feature.enabled {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RemoteSshDisabled,
            msg!("error-remote-ssh-disabled"),
        )
        .fact(Fact::source(&feature.source))
        .remediation(
            Remediation::text(msg!("remediation-remote-ssh-disabled"))
                .try_run("sbx settings get feature.ssh --json")
                .try_run("sbx settings set feature.ssh true"),
        ),
    ))
}
