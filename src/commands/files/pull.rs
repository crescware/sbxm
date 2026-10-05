use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{self, ConfigLocation, GlobalConfig, SandboxHomeRelativePath};
use crate::design::ProgressSink;
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
#[allow(clippy::too_many_arguments)]
pub fn pull(
    location: &ConfigLocation,
    config: &GlobalConfig,
    destination: &str,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
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
    progress.step(msg!(
        "progress-inspect-file",
        path = format!("{:?}", declaration.destination.as_path()),
        current = 1,
        total = 1
    ));
    let host_sha256 = files::read_source(declaration.source.as_path())?;

    let candidate = select::one(
        location,
        requested,
        &msg!("select-files-pull-heading"),
        prompt,
    )?;
    progress.step(msg!(
        "progress-project-lock",
        project = candidate.display_id()
    ));
    let locked = candidate.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    inventory::require_running(host, &locked.metadata, workspace_root, progress)?;
    // 置き換えた内容を広げる先があるかどうかを、結果の案内に使う。受け取ったあとに失敗
    // しうる工程を置かない。受け取ったものは、呼び出し側へ返してはじめて片付けられる。
    let others = select::candidates(location)?.len().saturating_sub(1);
    let sandbox = locked.metadata.sandbox_name();

    progress.step(msg!(
        "progress-inspect-file",
        path = format!("{:?}", declaration.destination.as_path()),
        current = 1,
        total = 1
    ));
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
        others,
        locked,
    })
}

#[cfg(test)]
#[path = "pull_test.rs"]
mod pull_test;
