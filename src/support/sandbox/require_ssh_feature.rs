use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{ssh_experiments_allowed, ssh_feature};

/// 実効設定が無効な場合だけ、有効化できる設定元かを区別して止める。
pub fn require_ssh_feature(host: &dyn HostEnvironment) -> Result<()> {
    let feature = ssh_feature(host)?;
    if !ssh_experiments_allowed(host)? {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RemoteSshRestricted,
                msg!("error-remote-ssh-restricted"),
            )
            .fact(Fact::field("platform.allowExperimentalFeatures"))
            .remediation(
                Remediation::text(msg!("remediation-remote-ssh-restricted"))
                    .try_run("sbx settings get platform.allowExperimentalFeatures --json"),
            ),
        ));
    }
    if feature.enabled {
        return Ok(());
    }
    let (id, description, remediation) = if feature.can_enable() {
        (
            ErrorId::RemoteSshDisabled,
            msg!("error-remote-ssh-disabled"),
            Remediation::text(msg!("remediation-remote-ssh-disabled"))
                .try_run("sbx settings get feature.ssh --json")
                .try_run("sbx settings set feature.ssh true"),
        )
    } else {
        (
            ErrorId::RemoteSshRestricted,
            msg!("error-remote-ssh-restricted"),
            Remediation::text(msg!("remediation-remote-ssh-restricted"))
                .try_run("sbx settings get feature.ssh --json"),
        )
    };
    Err(Error::single(
        Diagnostic::new(id, description)
            .fact(Fact::source(&feature.source))
            .remediation(remediation),
    ))
}
