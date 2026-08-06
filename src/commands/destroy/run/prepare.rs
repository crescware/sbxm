use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::Remediation;
use crate::support::daemon;
use crate::support::inventory::{self, ProjectState};
use crate::support::protection::{self, DestructiveOperation, ForceBypass, ProtectionRequest};
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

    let (worktrees, session_lease) = if force {
        // 保護ゲートとsession leaseを意図的に迂回する。通常経路のProtectionPermitとは
        // 別の型であり、architecture testがこの呼び出しの唯一性を確認する。
        let _bypass = ForceBypass::force_destroy();
        (Vec::new(), None)
    } else if state == ProjectState::NotCreated {
        (Vec::new(), None)
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
                    Remediation::text(msg!("remediation-destroy-force"))
                        .try_run(format!("sbxm destroy --force {}", metadata.display_id())),
                ),
            ));
        }
        // project lockを保持している間にexclusive session leaseを取り、最終protection
        // inspectからsandbox remove完了までこの`Prepared`が保持し続ける。この時点で
        // project lockは自分が排他的に保持しているため、取得できない原因は開いている
        // sessionのshared leaseだけである。
        let session_lease = locked.acquire_exclusive_session_lease()?;
        let layout = SandboxLayout::new(metadata.canonical_id());
        let request =
            ProtectionRequest::new(DestructiveOperation::Destroy, &name, &layout, metadata);
        let assessment = protection::gate::assess(host, request)?;
        let worktrees = assessment.worktrees().to_vec();
        // 通常経路のProtectionPermit。Step 1時点ではremove APIがまだこれを要求しない。
        protection::gate::authorize(assessment)?;
        (worktrees, Some(session_lease))
    };

    let plan = DestroyPlan {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        state,
        force,
        worktrees,
        removes: removes(&paths, &name, state),
        keeps: keeps(&paths),
        re_register: re_register(&paths, metadata)?,
    };

    Ok(Prepared {
        plan,
        paths,
        name,
        state,
        force,
        locked,
        _session_lease: session_lease,
    })
}
