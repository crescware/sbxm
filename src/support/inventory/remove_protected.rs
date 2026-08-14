use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;

use crate::design::ProgressSink;

use crate::support::protection::ProtectionPermit;
use crate::support::sandbox;

use super::{Poll, wait_until_absent};

/// 通常のrebuild/destroyがSandboxを削除し、一覧から消えるまで待つ。
///
/// 呼び出し側は、remove直前に取り直した観測への`ProtectionPermit`を渡す。削除する対象は
/// 引数では受け取らず、許可証が持つSandbox名だけを使う。共通ゲートを経ずに削除できる
/// 経路も、ある Sandbox への許可証で別の Sandbox を消す経路も、これで型から無くなる。
/// `permit`をconsumeするのは、1回発行した許可証を2回目のremoveへ使い回せないように
/// するためである。`destroy`は自前の確認も済ませている。`sbx`自身の確認は省かず、PTYの
/// 上でその確認にだけ答える。runtimeが示すactive-session拒否は、そのままcommand失敗
/// として伝わる。
#[allow(clippy::needless_pass_by_value)]
pub fn remove_protected(
    host: &dyn HostEnvironment,
    permit: ProtectionPermit,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let name = permit.sandbox();
    progress.step(msg!("progress-removing-sandbox"));
    let command = sandbox::remove_confirmed(name).timeout(TimeoutClass::SandboxLifecycle);
    host.run_pty_confirmed(&command)?.require_success()?;
    wait_until_absent(host, name, poll)
}
