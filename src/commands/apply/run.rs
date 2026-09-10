use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxState;
use crate::config::GlobalConfig;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use crate::design::ProgressSink;
use crate::design::Remediation;
use crate::project::SandboxLayout;
use crate::support::files::{self};
use crate::support::{
    daemon, disk, generation, inventory, provisioning, repository, sandbox, select,
};

use super::{ApplyOutput, Scope, Target};

/// 対象を引数またはpromptで解決し、構築済みの案件へ変更を適用する。
///
/// Sandboxの中身を変えるmutationであるため、対象を確かめた後にproject lockを取得し、
/// lock取得後のmetadataでpreconditionを判定し直してから適用する。世代交代の途中の案件も、
/// 初回構築が中断したままの案件も、hostの一覧を取る前にmetadataだけで拒否する。
pub fn run(
    target: Target,
    config: &GlobalConfig,
    scope: Scope,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ApplyOutput> {
    let Target {
        location,
        requested,
        prompt,
    } = target;
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        select::one(location, requested, &msg!("select-apply-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    // 中断した初回構築を暗黙に進めない。固定した入力snapshotで復旧するのは`open`または
    // `repair`であり、現在のconfigを正本にする`apply`が先に成果物を進めてよい状態ではない。
    // intentはmetadataだけで判定できるため、Sandboxの有無にも停止中かどうかにも依らず、
    // hostへ触れる前にここで1つに絞る。
    provisioning::require_no_initial_intent(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let entries = daemon::list(host)?;
    let Some(entry) = inventory::single(&entries, name.as_str())? else {
        return Err(inventory::not_created(&locked.metadata, name.as_str()));
    };

    sandbox::verify_identity(entry, &name, workspace_root)?;

    if entry.state != SandboxState::Running {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = entry.name,
                    observed = entry.state.as_str()
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-sandbox-not-running"))
                    .try_run(format!("sbxm open {}", locked.metadata.display_id())),
            ),
        ));
    }

    // sbxm自身がSandbox内を変更する工程が失敗した場合だけ、失敗直後の空き容量を
    // 追加のfactとして載せる。平常時はcommandを1つも増やさない。ここまでの検査で
    // `entry.state`は`Running`と確認済みである。
    let decorate = |error| disk::attach_on_failure(host, &entry.name, entry.state, error);

    let mut files = Vec::new();
    if scope.files {
        let inputs = provisioning::ProvisioningInputs::capture_files(&locked.paths, config)?;
        files = files::place_all(
            host,
            &entry.name,
            &inputs
                .iter()
                .map(|input| input.declaration.clone())
                .collect::<Vec<_>>(),
            files::Conflict::Overwrite,
        )
        .map_err(decorate)?;
        locked.metadata.declared_files = Some(provisioning::recorded_files(&inputs));
        metadata::update(&locked.paths, &locked.metadata)?;
    }

    let mut worktrees = None;
    if let Some(count) = scope.worktrees {
        raise_worktrees(&locked.paths, &mut locked.metadata, count)?;
        let layout = SandboxLayout::new(&canonical);
        let project = ProjectId::parse(&locked.metadata.display_id())?;
        repository::ensure_bare_clone(host, &entry.name, &project, &layout, progress)
            .map_err(decorate)?;
        let branch = repository::resolve_start_ref(
            host,
            &entry.name,
            &layout,
            &locked.paths,
            &mut locked.metadata,
        )?;
        repository::ensure_worktrees(
            host,
            &entry.name,
            &layout,
            &locked.metadata,
            &branch,
            progress,
        )
        .map_err(decorate)?;
        worktrees = Some(locked.metadata.provisioning.requested_worktrees);
    }

    Ok(ApplyOutput {
        project: locked.metadata.display_id(),
        sandbox: entry.name.clone(),
        files,
        worktrees,
    })
}

/// 目標worktree数を引き上げる。
///
/// 減らす指定は受け付けない。worktreeを減らすことはcheckoutされた作業を消すことであり、
/// `destroy`と同じ重さの確認が要る。
fn raise_worktrees(paths: &ProjectPaths, metadata: &mut ProjectMetadata, count: u32) -> Result<()> {
    let current = metadata.provisioning.requested_worktrees;
    if count < current {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::WorktreesNotReducible,
                msg!(
                    "error-worktrees-not-reducible",
                    project = metadata.display_id(),
                    requested = count,
                    current = current
                ),
            )
            .remediation(msg!("remediation-worktrees-not-reducible")),
        ));
    }
    if count == current {
        return Ok(());
    }
    metadata.provisioning.requested_worktrees = count;
    metadata::update(paths, metadata)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
