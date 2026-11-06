use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SshFeature;
use crate::diagnostics::Result;

/// 管理者側のgateは観測するだけとし、SSHの修復で変更しない。
pub(super) fn ssh_experiments_allowed(host: &dyn HostEnvironment) -> Result<bool> {
    super::ssh_setting(
        host,
        "platform.allowExperimentalFeatures",
        SshFeature::parse_experimental,
    )
}
