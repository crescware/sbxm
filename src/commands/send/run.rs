use std::collections::BTreeMap;
use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::repository::{self, SandboxOrigin, TagFollowing};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{generation, inventory, sandbox};

use super::{SendOutput, sent_changes};

/// 比べるSandboxのref。hostのbranchはremote-tracking refへ、tagは同じ名前へ届く。
const COMPARED: [&str; 2] = ["refs/remotes/origin/", "refs/tags/"];

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

    let before = origin_refs(host, sandbox_name.as_str(), &git_dir)?;
    origin.deliver(host, sandbox_name.as_str())?;
    repository::refresh_origin(
        host,
        sandbox_name.as_str(),
        &git_dir,
        TagFollowing::Auto,
        None,
    )?
    .require_success()?;
    let after = origin_refs(host, sandbox_name.as_str(), &git_dir)?;

    Ok(SendOutput {
        project: locked.metadata.display_id(),
        repository: repository.clone(),
        changes: sent_changes(&before, &after),
    })
}

/// Sandboxのremote-tracking refとtagの、ref名から先端への対応。
fn origin_refs(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    git_dir: &str,
) -> Result<BTreeMap<String, String>> {
    let mut args = vec![
        "git",
        "--git-dir",
        git_dir,
        "for-each-ref",
        "--format=%(refname) %(objectname)",
    ];
    args.extend(COMPARED);
    let listed = sandbox::exec(host, sandbox_name, &args)?.require_success()?;
    Ok(listed
        .stdout_text()
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(reference, tip)| (reference.to_string(), tip.to_string()))
        .collect())
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
