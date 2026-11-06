use super::{require_ssh_client, require_ssh_feature};
use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

/// client設定とSSH featureの実効設定を、接続せずに確かめる。
pub fn require_ssh(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    require_ssh_client(host, sandbox)?;
    require_ssh_feature(host)
}
