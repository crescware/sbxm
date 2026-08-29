use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::git;
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::SandboxLayout;

use crate::support::sandbox;

use super::{remote_default_branch, require_branch_name};

/// 起点となるbranchを確定させる。
///
/// hostのvalidationは、外部commandへ渡す前に確実に拒否できる条件だけを見る。
/// 起点として使う名前は、Sandbox内のgit自身にもう一度判定させてから採用する。
/// attached modeでremote default branchを解決した場合は、その場でmetadataへ記録する。
pub fn resolve_start_ref(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    paths: &ProjectPaths,
    project: &mut ProjectMetadata,
) -> Result<String> {
    let git_dir = layout.bare_git_dir();

    let stored = project.provisioning.start_ref.clone();
    let branch = match &stored {
        Some(branch) => branch.clone(),
        None => remote_default_branch(host, sandbox, &git_dir)?,
    };
    require_branch_name(host, sandbox, &branch)?;
    if stored.is_none() {
        project.provisioning.start_ref = Some(branch.clone());
        metadata::update(paths, project)?;
    }

    // tagやambiguous refを起点にしないよう、完全なremote-tracking refだけを確認する。
    let reference = git::origin_ref(&branch);
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "show-ref",
            "--verify",
            "--quiet",
            &reference,
        ],
    )?;
    if !outcome.success() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::StartRefUnresolved,
                msg!(
                    "error-start-ref-unresolved",
                    reference = reference,
                    project = project.display_id()
                ),
            )
            .remediation(msg!("remediation-start-ref-unresolved")),
        ));
    }
    Ok(branch)
}
