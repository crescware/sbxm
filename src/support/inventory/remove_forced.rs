use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use crate::design::ProgressSink;

use crate::support::sandbox;

use super::{Poll, wait_until_absent};

/// `destroy --force`だけがSandboxを削除する。データ保護検査と同じく、`sbx`自身の
/// 確認とactive-session検査も意図的に迂回する。
pub fn remove_forced(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-removing-sandbox"));
    let args = ["rm", "--force", name.as_str()];
    let command = sandbox::relayed(&args).timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;
    wait_until_absent(host, name, poll)
}
