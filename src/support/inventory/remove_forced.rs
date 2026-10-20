use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use crate::design::ProgressSink;

use crate::support::sandbox;

use super::{Poll, wait_until_absent};

/// `destroy --force`だけがSandboxを削除する。データ保護検査と同じく、daemonへの削除要求を
/// force=trueで送り、runtime自身のactive-session検査も意図的に迂回する。scoped secretは
/// この段では削除しない。
pub fn remove_forced(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-removing-sandbox"));
    sandbox::remove_preserving_secrets(host, name, true)?;
    wait_until_absent(host, name, poll)
}
