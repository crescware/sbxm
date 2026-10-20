use crate::boundary::host::HostEnvironment;
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
/// するためである。daemonへの削除要求はforce=falseで送り、runtime自身が示す
/// active-session拒否をそのまま失敗として伝える。scoped secretはこの段では削除しない。
#[allow(clippy::needless_pass_by_value)]
pub fn remove_protected(
    host: &dyn HostEnvironment,
    permit: ProtectionPermit,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let name = permit.sandbox();
    progress.step(msg!("progress-removing-sandbox"));
    sandbox::remove_preserving_secrets(host, name, false)?;
    wait_until_absent(host, name, poll)
}
