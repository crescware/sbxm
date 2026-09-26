use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::repository::{self, SandboxOrigin};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{bundle, generation, inventory};

use super::{SyncOutput, send_host_refs};

/// 対象を引数またはpromptで解決し、hostのrepositoryとSandboxのrepositoryを同期する。
///
/// 1. Sandboxのbranch、tag、各worktreeの`HEAD`を、hostの`refs/sbx/<sandbox>/`へ保存する
/// 2. 保存したbranchとtagを、hostのbranchとtagへ反映する。gitが断ったrefは動かさない
/// 3. hostのbranchとtagを、Sandboxのoriginへ送る
///
/// 2のあとで3を行うため、Sandboxのoriginは同期したあとのhostを映す。project lockを
/// 持って行い、動いているSandboxとだけ同期する。停止中のSandboxは起動しない。
pub fn run(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<SyncOutput> {
    let locked = select::one(location, requested, &msg!("select-sync-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    let origin = SandboxOrigin::of(&locked.metadata)?;
    let SandboxOrigin::Host { repository, .. } = &origin else {
        let project = locked.metadata.display_id();
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SyncRequiresLocal,
                msg!("error-sync-requires-local", project = project.clone()),
            )
            .remediation(
                Remediation::text(msg!("remediation-sync-requires-local"))
                    .try_run(format!("sbxm open {project}")),
            ),
        ));
    };
    inventory::require_running(host, &locked.metadata, workspace_root)?;
    let sandbox = locked.metadata.sandbox_name();
    let git_dir = SandboxLayout::new(locked.metadata.canonical_id()).bare_git_dir();
    // 構築が終わっていないSandboxとは同期しない。originを送る先も、保存する元も、この
    // 案件のbare repositoryである。
    repository::verify_bare_clone(host, sandbox.as_str(), &origin, &git_dir)?;

    let reflected = match bundle::save_to_host(host, &sandbox, &git_dir, repository)? {
        Some(_) => Some(bundle::reflect_saved(host, repository, sandbox.as_str())?),
        None => None,
    };
    let sent = send_host_refs(host, &origin, sandbox.as_str(), &git_dir)?;

    Ok(SyncOutput {
        project: locked.metadata.display_id(),
        repository: repository.clone(),
        namespace: sandbox.as_str().to_string(),
        reflected,
        sent,
    })
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
