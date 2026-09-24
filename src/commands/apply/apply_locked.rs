use std::path::{Component, Path, PathBuf};

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxState;
use crate::config::GlobalConfig;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::design::Remediation;
use crate::project::SandboxLayout;
use crate::support::files::{self, PlacedFile};
use crate::support::select::Locked;
use crate::support::{daemon, disk, generation, inventory, provisioning, repository, sandbox};

use super::{ApplyOutput, Scope};

/// lock済みの案件へ変更を適用する。
///
/// 世代交代の途中の案件も、初回構築が中断したままの案件も、hostの一覧を取る前に
/// metadataだけで拒否する。
pub(super) fn apply_locked(
    mut locked: Locked,
    config: &GlobalConfig,
    scope: Scope,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ApplyOutput> {
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
        files = apply_files(
            &mut locked,
            config,
            scope.force,
            host,
            &entry.name,
            &decorate,
        )?;
    }

    let mut worktrees = None;
    if let Some(count) = scope.worktrees {
        raise_worktrees(&locked.paths, &mut locked.metadata, count)?;
        let layout = SandboxLayout::new(&canonical);
        let origin = repository::SandboxOrigin::of(&locked.paths, &locked.metadata)?;
        repository::ensure_bare_clone(host, &entry.name, &origin, &layout, progress)
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

/// 宣言fileを置き、置いた1件ごとにbaselineを記録する。
///
/// baselineは、sbxmが各destinationへ最後に置いた内容である。それと異なるfileは
/// Sandboxの中で書き換えられたものであり、`force`が無ければ1件も置く前に拒否する。
/// 途中で失敗しても、それまでに置いたfileの記録は残す。記録が置いた内容より古いままだと、
/// 次の`apply`はsbxmが置いたfileをSandbox側の変更と読み違える。
fn apply_files(
    locked: &mut Locked,
    config: &GlobalConfig,
    force: bool,
    host: &dyn HostEnvironment,
    sandbox: &str,
    decorate: &dyn Fn(Error) -> Error,
) -> Result<Vec<PlacedFile>> {
    let inputs = provisioning::ProvisioningInputs::capture_files(&locked.paths, config)?;
    let declarations: Vec<_> = inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect();
    let mut baseline = locked.metadata.declared_files.clone().unwrap_or_default();
    let conflict = if force {
        files::Conflict::Overwrite
    } else {
        files::Conflict::Protect(&baseline)
    };
    let planned = files::plan_all(host, sandbox, &declarations, conflict).map_err(decorate)?;

    let mut placed = Vec::with_capacity(planned.len());
    for (file, input) in planned.iter().zip(&inputs) {
        let mut result = file.carry_out(host, sandbox).map_err(decorate)?;
        // 置いたのはsnapshotだが、利用者に示すのは宣言したfileである。
        result.source = PathBuf::from(&input.original_source);
        placed.push(result);
        let recorded = provisioning::recorded_file(input);
        if !baseline.contains(&recorded) {
            baseline.retain(|entry| !same_destination(&entry.destination, &recorded.destination));
            baseline.push(recorded);
            locked.metadata.declared_files = Some(baseline.clone());
            metadata::update(&locked.paths, &locked.metadata)?;
        }
    }

    // 宣言から外したfileの記録は残さない。そのfileはもう`apply`が置き換えない。
    let current = provisioning::recorded_files(&inputs);
    if locked.metadata.declared_files.as_ref() != Some(&current) {
        locked.metadata.declared_files = Some(current);
        metadata::update(&locked.paths, &locked.metadata)?;
    }
    Ok(placed)
}

/// 2つの記録が同じdestinationを指すか。`./`の有無のような綴りの違いは同じとみなす。
fn same_destination(left: &str, right: &str) -> bool {
    let parts = |value: &str| -> Vec<std::ffi::OsString> {
        Path::new(value)
            .components()
            .filter(|component| *component != Component::CurDir)
            .map(|component| component.as_os_str().to_os_string())
            .collect()
    };
    parts(left) == parts(right)
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
