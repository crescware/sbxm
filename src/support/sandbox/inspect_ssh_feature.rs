use super::{ssh_experiments_allowed, ssh_feature};
use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SshFeature;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// 管理者制約と設定元を確認し、有効化の判断にも使える観測結果を返す。
pub(crate) fn inspect_ssh_feature(host: &dyn HostEnvironment) -> Result<SshFeature> {
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
    if !feature.enabled && !feature.can_enable() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RemoteSshRestricted,
                msg!("error-remote-ssh-restricted"),
            )
            .fact(Fact::source(&feature.source))
            .remediation(
                Remediation::text(msg!("remediation-remote-ssh-restricted"))
                    .try_run("sbx settings get feature.ssh --json"),
            ),
        ));
    }
    Ok(feature)
}
