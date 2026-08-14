use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::protection::{self, ProtectionSnapshot};
use crate::support::{daemon, docker, generation, image, select};

use crate::commands::rebuild::Target;

use super::{Prepared, RebuildPlan, observe_protection};

/// 対象を引数またはpromptで解決し、削除計画を作る。
///
/// `RebuildIntent`がある場合はそこに固定した世代を、無い場合は現在のDockerfileを
/// 対象とする。Sandboxが無く`RebuildIntent`も無い場合は拒否する。返す`ProtectionSnapshot`
/// は、この削除計画の元になった最初の観測であり、`confirm`が明示確認と引き換えに
/// 消費する。
///
/// 停止しているSandboxの起動だけは、明示確認より前に行う唯一のhost状態の変更である。
/// 保存されていない作業は起動しなければ読めず、読めないまま削除計画を見せると、層Aも
/// 層Bも空の計画を「失うものは無い」と示すことになる。ここでcancelしてもSandboxは
/// 起動したまま残るが、image、template、metadata、`RebuildIntent`はどれも変えない。
/// destroyは同じ状況を`SandboxNotRunning`で拒否する。これから作り直す対象を起動する
/// rebuildと、利用者の判断を仰ぐdestroyの違いによる。
pub fn prepare(
    selection: Target,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<(Prepared, ProtectionSnapshot)> {
    let Target {
        location,
        requested,
        prompt,
    } = selection;
    // 対象が決まる前にhostの状態へ触れない。
    let locked =
        select::one(location, requested, &msg!("select-rebuild-heading"), prompt)?.lock()?;
    image::cleanup_stale_archives(&locked.paths);
    // project lockを保持している間にexclusive session leaseを取る。開いている
    // `sbxm open` sessionがあれば、削除計画を作る前にここで拒否する。この時点で
    // project lockは自分が排他的に保持しているため、取得できない原因は開いている
    // sessionのshared leaseだけである。
    let session_lease = locked.acquire_exclusive_session_lease()?;

    docker::require_reachable(host)?;

    let name = SandboxName::derive(locked.metadata.canonical_id());
    let current = generation::current_dockerfile_hash(&locked.paths)?;
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &locked.metadata, workspace_root)?;

    let target = if let Some(intent) = &locked.metadata.rebuild {
        intent.target_dockerfile_sha256.clone()
    } else {
        require_created(&locked.metadata, state, &name)?;
        current.clone()
    };

    let snapshot = observe_protection(
        host,
        &locked.metadata,
        &name,
        state,
        workspace_root,
        poll,
        progress,
    )?;
    // Blockerが1件でもあれば、削除計画を見せず明示確認も求めずにここで拒否する。
    protection::gate::require_no_blockers(snapshot.assessment())?;

    let plan = RebuildPlan {
        project: locked.metadata.display_id(),
        sandbox: name.as_str().to_string(),
        current_generation: current.clone(),
        target_generation: target.clone(),
        confirmable_losses: snapshot.assessment().confirmable_losses().to_vec(),
    };

    Ok((
        Prepared {
            plan,
            name,
            target,
            current,
            locked,
            _session_lease: session_lease,
        },
        snapshot,
    ))
}

/// `rebuild`は、Sandboxを持つか再構築が中断している案件だけを対象とする。
fn require_created(
    metadata: &ProjectMetadata,
    state: ProjectState,
    name: &SandboxName,
) -> Result<()> {
    match state {
        ProjectState::Running | ProjectState::Stopped => Ok(()),
        ProjectState::NotCreated => Err(inventory::not_created(metadata, name.as_str())),
    }
}
