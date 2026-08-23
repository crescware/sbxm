use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::run_exec;

/// `exec`と同じだが、`SandboxLifecycle`以外のtimeoutを使う呼び出し向け。
///
/// 応答が返らなくなりやすい状態を診断する用途など、600秒を待つべきでない場合に使う。
pub fn exec_with_timeout(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    timeout: TimeoutClass,
) -> Result<CommandOutcome> {
    run_exec(host, sandbox, None, args, timeout)
}
