use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::metadata::GitIdentity;
use crate::paths::ProjectParent;

use crate::commands::add::host_clone::HostClone;
use crate::design::ProgressSink;

use super::{AddOutput, AddRequest, register, was_already_registered};

/// 案件を管理下へ置き、host cloneを用意する。
///
/// Sandboxは作らない。Sandbox名はcanonical project IDから決まるため、この時点で
/// 確定して呼び出し側へ返せる。GitHub tokenの登録先がその名前になる。
pub fn run(
    location: &ConfigLocation,
    parent: &ProjectParent,
    request: &AddRequest,
    git_identity: &GitIdentity,
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<AddOutput> {
    let already_registered = was_already_registered(location, &request.repository)?;

    // Sandbox内で使うidentityは、案件を作る前に呼び出し側が決めている。
    let registration = register(location, parent, request, git_identity)?;
    // host cloneは、validation済みの入力と同じtransportとclone URLで取る。
    let clone = HostClone::ensure(
        host,
        &registration.paths,
        &registration.metadata.repository,
        progress,
    )?;

    let provisioning = &registration.metadata.provisioning;
    Ok(AddOutput {
        project: registration.metadata.display_id(),
        sandbox: registration.sandbox.as_str().to_string(),
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone(),
        requested_worktrees: provisioning.requested_worktrees,
        host_clone: clone.path,
        already_registered,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
