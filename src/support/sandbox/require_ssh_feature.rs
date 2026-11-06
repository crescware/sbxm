use super::{inspect_ssh_feature, require_ssh_feature_enabled};
use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

/// 有効なSSH設定を要求する。
pub fn require_ssh_feature(host: &dyn HostEnvironment) -> Result<()> {
    require_ssh_feature_enabled(&inspect_ssh_feature(host)?)
}
