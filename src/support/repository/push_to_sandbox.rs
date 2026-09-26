use std::collections::BTreeSet;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Result, unparseable};
use crate::support::sandbox;

use super::{
    PushRefusal, host_git, refusal_reason, sandbox_remote, sandbox_ssh_config, sandbox_unwritable,
};

/// hostのbranchが届く、Sandboxのremote-tracking ref。
const ORIGIN_BRANCHES: &str = "refs/remotes/origin/";

/// hostの`repository`のbranchとtagを、`sandbox`のoriginへpushする。gitが断ったrefを返す。
///
/// Sandboxの中で`git fetch --prune origin`をしたときと同じものを、hostから書き込む。
/// branchは`refs/remotes/origin/*`へ強制して置き、hostで消したbranchはSandboxからも
/// 消す。tagは同じ名前へ置き、上書きしない。Sandboxで作ったtagを、hostに無いことを
/// 理由に消さないよう、tagは別のpushで送る。
///
/// 消すbranchは、pushの前にSandboxのoriginを読んで決める。`git push --prune`は使わない。
/// `refs/remotes/origin/HEAD`のようなsymrefまで消そうとし、受け取るgitはsymrefを辿って
/// 指す先のbranchを消す。`git fetch --prune`と同じく、symrefは消さずに残す。
///
/// 送る側の`pre-push`は、hostのrepositoryから外へ出すときの確認であり、sbxmがSandboxの
/// originを最新にするときには走らせない。
pub fn push_to_sandbox(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
    git_dir: &str,
) -> Result<Vec<PushRefusal>> {
    let branches = format!("+refs/heads/*:{ORIGIN_BRANCHES}*");
    let stale = stale_origin_branches(host, repository, sandbox, git_dir)?
        .into_iter()
        .map(|reference| format!(":{reference}"));
    let refspecs: Vec<String> = std::iter::once(branches).chain(stale).collect();
    let refspecs: Vec<&str> = refspecs.iter().map(String::as_str).collect();
    let mut refused = push(host, repository, sandbox, git_dir, &refspecs)?;
    refused.extend(push(
        host,
        repository,
        sandbox,
        git_dir,
        &["refs/tags/*:refs/tags/*"],
    )?);
    Ok(refused)
}

fn push(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
    git_dir: &str,
    tail: &[&str],
) -> Result<Vec<PushRefusal>> {
    let remote = sandbox_remote(sandbox, git_dir);
    let ssh = sandbox_ssh_config();
    let mut args = vec![
        "-c",
        ssh.as_str(),
        "push",
        "--porcelain",
        "--no-verify",
        remote.as_str(),
    ];
    args.extend_from_slice(tail);
    let pushed = host_git(
        host,
        repository,
        &args,
        None,
        TimeoutClass::RepositoryTransfer,
    )?;
    // 断ったrefがあれば1で終わる。それ以外の失敗は、refごとの答えを持たない。
    if !matches!(pushed.status.code(), Some(0 | 1)) {
        return Err(sandbox_unwritable(sandbox, &pushed));
    }
    let mut refused = Vec::new();
    for line in pushed.stdout_text().lines() {
        let mut fields = line.split('\t');
        let (Some("!"), Some(refspec), Some(summary)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some((_, reference)) = refspec.split_once(':') else {
            return Err(unparseable(
                "git push",
                "a ref line had no source and destination",
            ));
        };
        // Sandboxが断った理由は、Sandboxのgitやhookが決めた文字列である。
        refused.push(PushRefusal {
            reference: reference.to_string(),
            reason: sandbox::neutralized(refusal_reason(summary)),
        });
    }
    // 断ったrefが1つも読めない失敗は、refごとの答えではない。
    if refused.is_empty() && !pushed.success() {
        return Err(sandbox_unwritable(sandbox, &pushed));
    }
    Ok(refused)
}

/// Sandboxのoriginにあり、hostの`repository`にもうbranchが無いremote-tracking ref。
///
/// symrefは数えない。消そうとすると、受け取るgitはsymrefを辿って指す先のbranchを消す。
fn stale_origin_branches(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
    git_dir: &str,
) -> Result<Vec<String>> {
    let listed = host_git(
        host,
        repository,
        &["for-each-ref", "--format=%(refname)", "refs/heads/"],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?
    .stdout_text();
    let branches: BTreeSet<&str> = listed
        .lines()
        .filter_map(|line| line.strip_prefix("refs/heads/"))
        .collect();
    let origin = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "for-each-ref",
            "--format=%(refname) %(symref)",
            ORIGIN_BRANCHES,
        ],
    )?;
    Ok(origin
        .lines()
        .map(|line| line.split_once(' ').unwrap_or((line, "")))
        .filter(|(_, symref)| symref.is_empty())
        .filter_map(|(reference, _)| {
            let branch = reference.strip_prefix(ORIGIN_BRANCHES)?;
            (!branches.contains(branch)).then(|| reference.to_string())
        })
        .collect())
}
