use crate::boundary::host::protocol::SandboxEntry;
use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::boundary::os::SystemClock;
use crate::diagnostics::Result;

/// 指定したtimeoutで現在のSandbox一覧を読む。daemon起動直後の不完全な出力は再試行する。
///
/// 読み直すまでの待ちはOSの時計で待つ。一覧を読む経路は時計を持たないため、ここで束ねる。
pub fn list_with_timeout(
    host: &dyn HostEnvironment,
    timeout: TimeoutClass,
) -> Result<Vec<SandboxEntry>> {
    super::list_retrying(host, timeout, &SystemClock)
}
