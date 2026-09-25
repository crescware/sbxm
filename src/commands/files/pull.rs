use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{self, ConfigLocation, GlobalConfig, SandboxHomeRelativePath};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::project::ProjectId;
use crate::support::files;
use crate::support::generation;
use crate::support::inventory;
use crate::support::select::{self, ProjectPrompt};

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
    inventory::require_running(host, &locked.metadata, workspace_root)?;
    let sandbox = locked.metadata.sandbox_name();

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
    // 置き換えた内容を広げる先があるかどうかを、結果の案内に使う。
    let others = select::candidates(location)?.len().saturating_sub(1);
    Ok(Pulled {
        declaration,
        project: locked.metadata.display_id(),
        copy,
        host_sha256,
        others,
        locked,
    })
}

#[cfg(test)]
#[path = "pull_test.rs"]
mod pull_test;
