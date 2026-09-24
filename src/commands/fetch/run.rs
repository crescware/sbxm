use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{bundle, generation, inventory, repository};

use super::FetchOutput;

/// 対象を引数またはpromptで解決し、そのSandboxのcommitをhostへ保存する。
///
/// hostのrepositoryの名前空間を書き換えるため、project lockを持って行う。動いている
/// Sandboxからだけ受け取り、停止中のSandboxを起動しない。
pub fn run(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<FetchOutput> {
    let locked = select::one(location, requested, &msg!("select-fetch-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    inventory::require_running(host, &locked.metadata, workspace_root)?;
    let sandbox = locked.metadata.sandbox_name();
    let target = repository::host_repository(&locked.paths, &locked.metadata);
    let changes = bundle::save_to_host(
        host,
        &locked.paths,
        &sandbox,
        &SandboxLayout::new(locked.metadata.canonical_id()).bare_git_dir(),
        &target,
    )?;
    Ok(FetchOutput {
        project: locked.metadata.display_id(),
        repository: target,
        namespace: sandbox.as_str().to_string(),
        changes,
    })
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
