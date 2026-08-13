use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::Remediation;
use crate::support::daemon;
use crate::support::inventory::{self, ProjectState};
use crate::support::protection::{self, Unmanaged};
use crate::support::select::{self, ProjectPrompt};

use super::{DestroyPlan, Prepared, keeps, re_register, removes};

/// 対象を特定し、削除して良い状態であることを確かめる。
///
/// 削除そのものはrecordを消すだけであり、中立workspace directoryを必要としない。
/// 停止中のSandboxを通常modeで断るのは、中を観測できないからであり、これはdirectory
/// が在っても同じである。runningのSandboxは、データ保護検査の入口でdirectoryの実在を
/// 確かめる。mount元が無いままでは、中を見るcommandの答えを信頼できないためである。
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

    let worktrees = if force || state == ProjectState::NotCreated {
        Vec::new()
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
        let layout = SandboxLayout::new(metadata.canonical_id());
        protection::inspect(
            host,
            &name,
            workspace_root,
            &layout,
            metadata,
            Unmanaged::Allowed,
        )?
        .worktrees
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
    })
}
