use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxEntry;
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::support::login;

use super::require_supported_version;

/// Docker Sandboxesを使うcommandが、案件の選択や確認より先に通す確認。
///
/// CLIのversionを確かめてから、loginを確かめる。loginのprobeは`sbx ls --json`の出力を
/// 読むため、古いCLIでは未loginではなく「出力を解釈できない」として止まってしまう。
///
/// loginのprobeで読んだSandboxの一覧を返す。使ってよい場面は`login::require_signed_in`
/// が決める。
pub fn require_ready(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<SandboxEntry>> {
    require_supported_version(location, host, progress)?;
    login::require_signed_in(host, progress)
}
