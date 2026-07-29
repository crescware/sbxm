//! 全managed worktreeの起点にするbranchの決定。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::git;
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::SandboxLayout;

use super::sandbox;

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

/// 起点branch名を、Sandbox内の`git check-ref-format --branch`で再検証する。
///
/// repositoryを指定せずに実行するため、`@{-1}`のような文脈依存の短縮形は展開されず、
/// branch名としてそのまま判定される。
pub(super) fn require_branch_name(
    host: &dyn HostEnvironment,
    sandbox: &str,
    branch: &str,
) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "check-ref-format", "--branch", branch],
    )?;
    if outcome.success() {
        return Ok(());
    }
    fail(
        ErrorId::InvalidBranchName,
        msg!(
            "error-invalid-branch-name",
            value = branch,
            detail = "git in the sandbox does not accept this as a branch name"
        ),
    )
}

/// `git ls-remote --symref origin HEAD`が示すdefault branch。
pub(super) fn remote_default_branch(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
) -> Result<String> {
    let output = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "ls-remote",
            "--symref",
            "origin",
            "HEAD",
        ],
    )?;

    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("ref:") else {
            continue;
        };
        let Some(reference) = rest.split_whitespace().next() else {
            continue;
        };
        if let Some(branch) = reference.strip_prefix("refs/heads/")
            && git::validate_branch_name(branch).is_ok()
        {
            return Ok(branch.to_string());
        }
    }

    Err(Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = "git ls-remote --symref",
            detail = "no branch was reported for HEAD"
        ),
    ))
}

#[cfg(test)]
#[path = "start_ref_test.rs"]
mod start_ref_test;
