use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxLayout;

use crate::design::ProgressSink;
use crate::support::daemon;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::protection::{self, DestructiveOperation, Request};
use crate::support::select;

use crate::commands::destroy::Selection;

use super::{DestroyPlan, Prepared, keeps, re_register, removes};

/// 対象を特定し、削除して良い状態であることを確かめる。
///
/// 削除そのものはrecordを消すだけであり、中立workspace directoryを必要としない。
/// runningのSandboxは、データ保護検査の入口でdirectoryの実在を確かめる。mount元が
/// 無いままでは、中を見るcommandの答えを信頼できないためである。
///
/// 停止しているSandboxは、削除計画を作るためにここで起動する。中を観測しなければ、
/// 失うものが無いのか観測できていないだけなのかを区別できず、空の計画を「失うものは
/// 無い」として見せることになる。起動は`open`と違い、repositoryの取得もworktreeの
/// 用意も行わない。これから消す案件のために、利用者へ準備をやり直させない。
/// 確認をcancelしてもSandboxは起動したまま残るが、metadataもregistryも変えない。
pub fn prepare(
    selection: Selection,
    force: bool,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<Prepared> {
    let Selection {
        location,
        requested,
        prompt,
    } = selection;
    // 対象が決まる前にhostの状態へ触れない。
    let locked =
        select::one(location, requested, &msg!("select-destroy-heading"), prompt)?.lock()?;
    let paths = locked.paths.clone();

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();
    let entries = daemon::list(host)?;
    let observed = inventory::state_of(&entries, metadata, workspace_root)?;

    let (worktrees, confirmable_losses, snapshot, session_lease, started) = if force {
        // `--force`は保護ゲートとsession leaseを意図的に迂回する別操作であり、通常経路の観測は行わない。
        // 観測しない以上、観測のための起動も行わない。
        (Vec::new(), Vec::new(), None, None, false)
    } else {
        // Sandboxがそもそも無ければ、session leaseを取る対象も観測する対象も無い。
        // それ以外はproject lockを保持している間にexclusive session leaseを取り、
        // 最終protection inspectからsandbox remove完了までこの`Prepared`が保持し
        // 続ける。この時点でproject lockは自分が排他的に保持しているため、取得できない
        // 原因は開いているsessionのshared leaseだけである。
        let session_lease = if observed == ProjectState::NotCreated {
            None
        } else {
            Some(locked.acquire_exclusive_session_lease()?)
        };
        // 停止中のSandboxは、明示確認より前に行う唯一のhost状態の変更として起動する。
        let started = observed == ProjectState::Stopped;
        if started {
            inventory::start(host, metadata, workspace_root, progress)?;
            inventory::wait_until_running(host, metadata, workspace_root, poll)?;
        }
        let snapshot = if observed == ProjectState::NotCreated {
            protection::gate::assess_absent(
                DestructiveOperation::Destroy,
                metadata.display_id(),
                &name,
            )
        } else {
            let layout = SandboxLayout::new(metadata.canonical_id());
            let request = Request::new(
                DestructiveOperation::Destroy,
                &name,
                workspace_root,
                &layout,
                metadata,
            );
            protection::gate::assess(host, &request)?
        };
        // Blockerが1件でもあれば、削除計画を見せず明示確認も求めずにここで拒否する。
        protection::gate::require_no_blockers(snapshot.assessment())?;
        let worktrees = snapshot.assessment().worktrees().to_vec();
        let confirmable_losses = snapshot.assessment().confirmable_losses().to_vec();
        (
            worktrees,
            confirmable_losses,
            Some(snapshot),
            session_lease,
            started,
        )
    };

    // 起動を待ち切った直後の状態はrunningである。観測した時点の停止を計画へ残すと、
    // 計画が示すstateと、これから削除する対象の状態が食い違う。
    let state = if started {
        ProjectState::Running
    } else {
        observed
    };

    let plan = DestroyPlan {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        state,
        started,
        force,
        worktrees,
        confirmable_losses,
        removes: removes(&paths, &name, state),
        keeps: keeps(&paths),
        re_register: re_register(&paths, metadata)?,
    };

    Ok(Prepared {
        plan,
        paths,
        name,
        state,
        workspace_root: workspace_root.to_path_buf(),
        locked,
        snapshot,
        _session_lease: session_lease,
    })
}
