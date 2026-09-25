use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::project::SandboxName;
use crate::support::repository::host_git;

use super::{SavedTip, saved_namespace};

/// hostの`repository`が`refs/sbx/<sandbox>/`に持つ、保存済みのrefの先端。
///
/// 退避した`archive/`の先端も含める。どれもhostに残り、Sandboxが消えても失われない。
pub fn saved_tips(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &SandboxName,
) -> Result<Vec<SavedTip>> {
    let listed = host_git(
        host,
        repository,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            &saved_namespace(sandbox.as_str()),
        ],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?
    .stdout_text();
    Ok(listed
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(reference, commit)| SavedTip {
            reference: reference.to_string(),
            commit: commit.to_string(),
        })
        .collect())
}
