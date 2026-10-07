use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{generation, host_sync, inventory, repository};
use crate::time::Clock;

use super::SaveOutput;

/// 対象の案件のSandboxのcommitを、hostのrepositoryの`refs/sbx/<sandbox>/`へ保存する。
///
/// 保存の申し出に答えたときに使う。hostのbranchとtagは動かさない。hostのrepositoryの
/// 名前空間を書き換えるため、project lockを取り直して行う。動いているSandboxからだけ
/// 受け取り、停止中のSandboxを起動しない。
pub(super) fn save_now(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    clock: &dyn Clock,
    progress: &mut dyn ProgressSink,
) -> Result<SaveOutput> {
    let candidate = select::one(location, requested, &msg!("select-save-heading"), prompt)?;
    progress.step(msg!(
        "progress-project-lock",
        project = candidate.display_id()
    ));
    let locked = candidate.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    inventory::require_running(host, &locked.metadata, workspace_root, progress)?;
    let sandbox = locked.metadata.sandbox_name();
    let target = repository::host_repository(&locked.paths, &locked.metadata);
    progress.step(msg!(
        "progress-saving-to-host",
        project = locked.metadata.display_id()
    ));
    let changes = host_sync::save_to_host(
        host,
        &sandbox,
        &SandboxLayout::new(locked.metadata.canonical_id()).bare_git_dir(),
        &target,
        clock.wall(),
    )?;
    Ok(SaveOutput {
        project: locked.metadata.display_id(),
        repository: target,
        namespace: sandbox.as_str().to_string(),
        changes,
    })
}

#[cfg(test)]
#[path = "save_now_test.rs"]
mod save_now_test;
