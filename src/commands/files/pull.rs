use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{self, ConfigLocation, GlobalConfig, SandboxHomeRelativePath};
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::project::ProjectId;
use crate::support::files;
use crate::support::inventory::{self, ProjectState};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{daemon, generation};

use super::{Pulled, invalid_destination};

/// 宣言fileのSandbox側の内容を、案件の隔離領域へ取り出す。
///
/// 動いているSandboxからだけ取り出し、停止中のSandboxを起動しない。取り出すだけで、host
/// の宣言fileもbaselineも変えない。
pub fn pull(
    location: &ConfigLocation,
    config: &GlobalConfig,
    destination: &str,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Pulled> {
    let destination = SandboxHomeRelativePath::new(destination)
        .map_err(|reason| invalid_destination(destination, reason))?;
    let Some(declaration) = config
        .files
        .iter()
        .find(|declared| declared.destination.names_same_place(&destination))
        .cloned()
    else {
        return Err(config::file_not_declared(&destination));
    };
    let host_sha256 = files::read_source(declaration.source.as_path())?;

    let locked = select::one(
        location,
        requested,
        &msg!("select-files-pull-heading"),
        prompt,
    )?
    .lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    let sandbox = locked.metadata.sandbox_name();
    let entries = daemon::list(host)?;
    match inventory::state_of(&entries, &locked.metadata, workspace_root)? {
        ProjectState::Running => {}
        ProjectState::NotCreated => {
            return Err(inventory::not_created(&locked.metadata, sandbox.as_str()));
        }
        ProjectState::Stopped => {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotRunning,
                    msg!(
                        "error-sandbox-not-running",
                        sandbox = sandbox.as_str(),
                        observed = ProjectState::Stopped.as_str()
                    ),
                )
                .remediation(
                    Remediation::text(msg!("remediation-sandbox-not-running"))
                        .try_run(format!("sbxm open {}", locked.metadata.display_id())),
                ),
            ));
        }
    }

    let Some(copy) = files::receive_copy(
        host,
        sandbox.as_str(),
        &declaration,
        &locked.paths.incoming_dir(),
    )?
    else {
        return Err(Error::single(Diagnostic::new(
            ErrorId::FileNotInSandbox,
            msg!(
                "error-file-not-in-sandbox",
                sandbox = sandbox.as_str(),
                destination = paths::display(declaration.destination.as_path())
            ),
        )));
    };
    Ok(Pulled {
        declaration,
        project: locked.metadata.display_id(),
        copy,
        host_sha256,
        locked,
    })
}

#[cfg(test)]
#[path = "pull_test.rs"]
mod pull_test;
