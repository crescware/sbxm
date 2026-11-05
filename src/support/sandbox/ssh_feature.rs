use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SshFeature;
use crate::diagnostics::Result;

/// daemonが動いている場合だけ呼ぶ。設定の読み取り自体がdaemonを起動し得る。
pub fn ssh_feature(host: &dyn HostEnvironment) -> Result<SshFeature> {
    super::ssh_setting(host, "feature.ssh", SshFeature::parse)
}
