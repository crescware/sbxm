use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::commands::sync::send_host_refs;
use crate::config::ConfigLocation;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::repository::{self, SandboxOrigin};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{generation, inventory};

use super::SendOutput;

/// 対象を引数またはpromptで解決し、hostのbranchとtagをSandboxのoriginへ送る。
///
/// Sandboxのoriginが読むbundleを送り直し、Sandboxの中で`git fetch --prune origin`を
/// 行う。worktreeとbranchには触れず、取り込むかどうかはSandboxの中で決める。project
/// lockを持って行い、動いているSandboxへだけ送る。停止中のSandboxは起動しない。
pub fn run(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<SendOutput> {
    let locked = select::one(location, requested, &msg!("select-send-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    let origin = SandboxOrigin::of(&locked.paths, &locked.metadata)?;
    let SandboxOrigin::Host { repository, .. } = &origin else {
        let project = locked.metadata.display_id();
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SendRequiresLocal,
                msg!("error-send-requires-local", project = project.clone()),
            )
            .remediation(
                Remediation::text(msg!("remediation-send-requires-local"))
                    .try_run(format!("sbxm open {project}")),
            ),
        ));
    };
    inventory::require_running(host, &locked.metadata, workspace_root)?;
    let sandbox_name = locked.metadata.sandbox_name();
    let git_dir = SandboxLayout::new(locked.metadata.canonical_id()).bare_git_dir();
    // 送ったbundleを読む入れ物が、この案件のbare repositoryであることを先に確かめる。
    // 構築が終わっていないSandboxへ置くと、repositoryになる前の場所を塞ぐ。
    repository::verify_bare_clone(host, sandbox_name.as_str(), &origin, &git_dir)?;

    let changes = send_host_refs(host, &origin, sandbox_name.as_str(), &git_dir)?;

    Ok(SendOutput {
        project: locked.metadata.display_id(),
        repository: repository.clone(),
        changes,
    })
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
