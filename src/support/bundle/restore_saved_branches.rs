use std::collections::BTreeSet;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::support::repository::host_git;
use crate::support::sandbox;

use super::{saved_namespace, send_to_sandbox};

/// hostへ保存したbranchを、作り直したSandboxのbranchとして戻す。
///
/// hostの`refs/sbx/<sandbox>/heads/*`を1つのbundleにして送り、Sandboxの中で同じ名前の
/// `refs/heads/*`へ取り込む。originに同じ名前のbranchがあれば、それをupstreamにする。
/// 送ったbundleは取り込んだあとに消す。保存したbranchが無ければ何もしない。
///
/// Sandboxのbare repositoryはoriginを読み終え、まだbranchを持たないものとする。戻した
/// branchの名前を返す。
pub fn restore_saved_branches(
    host: &dyn HostEnvironment,
    repository: &Path,
    staging: &Path,
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

    let bundle = format!("{git_dir}/sbxm/restore.bundle");
    send_to_sandbox(
        host,
        repository,
        &[&format!("--glob={saved}*")],
        staging,
        sandbox_name,
        &bundle,
    )?;
    let fetched = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "fetch",
            "--quiet",
            "--no-tags",
            &bundle,
            &format!("{saved}*:refs/heads/*"),
        ],
    );
    // 取り込めたかどうかにかかわらず、送ったbundleは残さない。
    let removed = sandbox::exec(host, sandbox_name, &["rm", "-f", &bundle]);
    fetched?.require_success()?;
    removed?.require_success()?;

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
