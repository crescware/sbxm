use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::Remediation;
use crate::support::daemon;
use crate::support::inventory::{self, ProjectState};
use crate::support::protection::{self, DestructiveOperation, Request};
use crate::support::select::{self, ProjectPrompt};

use super::{DestroyPlan, Prepared, keeps, re_register, removes};

/// 対象を特定し、削除して良い状態であることを確かめる。
pub fn prepare(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    force: bool,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
) -> Result<Prepared> {
    // 対象が決まる前にhostの状態へ触れない。
    let locked =
        select::one(location, requested, &msg!("select-destroy-heading"), prompt)?.lock()?;
    let paths = locked.paths.clone();

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, metadata, workspace_root)?;

    let (worktrees, confirmable_losses, snapshot, session_lease) = if force {
        // `--force`は保護ゲートとsession leaseを意図的に迂回する別操作であり、通常経路の観測は行わない。
        (Vec::new(), Vec::new(), None, None)
    } else {
        if state == ProjectState::Stopped {
            // 停止中のSandboxは内部を観測できないため、通常modeでは削除しない。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotRunning,
                    msg!(
                        "error-sandbox-not-running",
                        sandbox = name,
                        observed = "stopped"
                    ),
                )
                .remediation(
                    Remediation::text(msg!("remediation-destroy-stopped"))
                        .try_run(format!("sbxm open {}", metadata.display_id())),
                ),
            ));
        }
        // Sandboxがそもそも無ければ、session leaseを取る対象も観測する対象も無い。
        // それ以外はproject lockを保持している間にexclusive session leaseを取り、
        // 最終protection inspectからsandbox remove完了までこの`Prepared`が保持し
        // 続ける。この時点でproject lockは自分が排他的に保持しているため、取得できない
        // 原因は開いているsessionのshared leaseだけである。
        let session_lease = if state == ProjectState::NotCreated {
            None
        } else {
            Some(locked.acquire_exclusive_session_lease()?)
        };
        let snapshot = if state == ProjectState::NotCreated {
            protection::gate::assess_absent(
                DestructiveOperation::Destroy,
                metadata.display_id(),
                &name,
            )
        } else {
            let layout = SandboxLayout::new(metadata.canonical_id());
            let request = Request::new(DestructiveOperation::Destroy, &name, &layout, metadata);
            protection::gate::assess(host, &request)?
        };
        // Blockerが1件でもあれば、削除計画を見せず明示確認も求めずにここで拒否する。
        protection::gate::require_no_blockers(snapshot.assessment())?;
        let worktrees = snapshot.assessment().worktrees().to_vec();
        let confirmable_losses = snapshot.assessment().confirmable_losses().to_vec();
        (worktrees, confirmable_losses, Some(snapshot), session_lease)
    };

    let plan = DestroyPlan {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        state,
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
        locked,
        snapshot,
        _session_lease: session_lease,
    })
}
