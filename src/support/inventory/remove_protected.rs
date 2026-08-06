use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use crate::design::ProgressSink;

use crate::support::sandbox;

use super::{Poll, wait_until_absent};

/// 通常のrebuild/destroyがSandboxを削除し、一覧から消えるまで待つ。
///
/// 削除してよいかはsbxmが先に判定しており、`destroy`は自前の確認も済ませている。
/// `sbx`自身の確認は省かず、PTYの上でその確認にだけ答える。runtimeが示す
/// active-session拒否は、そのままcommand失敗として伝わる。
pub fn remove_protected(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-removing-sandbox"));
    let command = sandbox::remove_confirmed(name).timeout(TimeoutClass::SandboxLifecycle);
    host.run_pty_confirmed(&command)?.require_success()?;
    wait_until_absent(host, name, poll)
}
