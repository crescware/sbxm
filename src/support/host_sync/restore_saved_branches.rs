use std::collections::BTreeSet;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::support::repository::{
    host_git, sandbox_remote, sandbox_ssh_config, sandbox_unwritable,
};
use crate::support::sandbox;

use super::saved_namespace;

/// hostへ保存したbranchを、作り直したSandboxのbranchとして戻す。
///
/// hostのgitが、`sbxm open`と同じsshでSandboxのrepositoryへ、`refs/sbx/<sandbox>/heads/*`
/// を同じ名前の`refs/heads/*`としてpushする。originに同じ名前のbranchがあれば、それを
/// upstreamにする。保存したbranchが無ければ何もしない。
///
/// Sandboxのbare repositoryはoriginを読み終え、まだbranchを持たないものとする。戻した
/// branchの名前を返す。
pub fn restore_saved_branches(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox_name: &str,
    git_dir: &str,
) -> Result<Vec<String>> {
    let saved = format!("{}heads/", saved_namespace(sandbox_name));
    let listed = host_git(
        host,
        repository,
        &["for-each-ref", "--format=%(refname)", &saved],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?;
    let branches: Vec<String> = listed
        .stdout_text()
        .lines()
        .filter_map(|line| line.strip_prefix(&saved))
        .map(str::to_owned)
        .collect();
    if branches.is_empty() {
        return Ok(branches);
    }

    let remote = sandbox_remote(sandbox_name, git_dir);
    let ssh = sandbox_ssh_config();
    let refspec = format!("{saved}*:refs/heads/*");
    // 送る側の`pre-push`は、hostのrepositoryから外へ出すときの確認であり、sbxmが
    // Sandboxへ戻すときには走らせない。
    let pushed = host_git(
        host,
        repository,
        &[
            "-c",
            &ssh,
            "push",
            "--quiet",
            "--no-verify",
            &remote,
            &refspec,
        ],
        None,
        TimeoutClass::RepositoryTransfer,
    )?;
    if !pushed.success() {
        return Err(sandbox_unwritable(sandbox_name, &pushed));
    }

    let tracked = remote_branches(host, sandbox_name, git_dir)?;
    for branch in branches.iter().filter(|branch| tracked.contains(*branch)) {
        sandbox::exec(
            host,
            sandbox_name,
            &[
                "git",
                "--git-dir",
                git_dir,
                "branch",
                "--quiet",
                &format!("--set-upstream-to=refs/remotes/origin/{branch}"),
                branch,
            ],
        )?
        .require_success()?;
    }
    Ok(branches)
}

/// Sandboxのoriginが持つbranchの名前。
fn remote_branches(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    git_dir: &str,
) -> Result<BTreeSet<String>> {
    let prefix = "refs/remotes/origin/";
    let listed = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "for-each-ref",
            "--format=%(refname)",
            prefix,
        ],
    )?
    .require_success()?;
    Ok(listed
        .stdout_text()
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(str::to_owned)
        .collect())
}
