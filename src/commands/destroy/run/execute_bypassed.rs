use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::daemon;
use crate::support::inventory::{self, Poll, ProjectState};

use super::{DestroyOutcome, Prepared, finish_removal};

/// 確認手順を通らずにSandboxを削除する。`--force`だけがここを通る。
///
/// `--force`は保護ゲートを意図的に迂回する別操作である。Sandboxがそもそも無い場合は
/// 削除commandを実行せず、不在を1回確かめてから管理情報の後始末へ進む。
pub fn execute_bypassed(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<DestroyOutcome> {
    if prepared.state == ProjectState::NotCreated {
        // 削除commandを実行しない場合だけ、一覧で不在を1回確かめる。
        require_absent(host, &prepared.name)?;
    } else {
        // データ保護検査と同じく、sbx自身の確認とactive-session検査も意図的に迂回する。
        inventory::remove_forced(host, &prepared.name, poll, progress)?;
    }
    finish_removal(host, prepared)
}

/// Sandboxが存在しないことを1回確認する。
fn require_absent(host: &dyn HostEnvironment, name: &SandboxName) -> Result<()> {
    if inventory::single(&daemon::list(host)?, name.as_str())?.is_some() {
        return Err(inventory::still_present(name));
    }
    Ok(())
}
