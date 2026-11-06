use crate::boundary::host::protocol::SandboxEntry;
use crate::boundary::host::{CommandSpec, HostEnvironment, SignalGuard, TimeoutClass};
use crate::boundary::os::{RealHost, SystemClock};
use crate::design::ProgressSink;
use crate::diagnostics::Result;

/// 指定したtimeoutで現在のSandbox一覧を読む。daemon起動直後の不完全な出力は再試行する。
/// Probeでは一時的なlogin service障害と時間切れも再試行し、待機中もCtrl-Cを見張る。
///
/// 読み直すまでの待ちはOSの時計で待つ。一覧を読む経路は時計を持たないため、ここで束ねる。
pub fn list_with_timeout(
    host: &dyn HostEnvironment,
    timeout: TimeoutClass,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::probe("sbx", &["ls", "--json"]);
    SignalGuard::watching(&RealHost, &spec, |interrupted| {
        super::list_retrying(host, timeout, &SystemClock, progress, interrupted)
    })
}
