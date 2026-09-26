use std::collections::BTreeMap;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::support::repository::SandboxOrigin;
use crate::support::sandbox;

use super::{SentChange, sent_changes};

/// 比べるSandboxのref。hostのbranchはremote-tracking refへ、tagは同じ名前へ届く。
const COMPARED: [&str; 2] = ["refs/remotes/origin/", "refs/tags/"];

/// hostのbranchとtagを、`sandbox`のoriginへ送る。Sandboxのorigin側で変わったrefと、
/// gitが断ったrefを返す。
///
/// hostのgitが、ssh越しにSandboxのoriginを書き込む。worktreeとbranchには触れず、
/// 取り込むかどうかはSandboxの中で決める。`git_dir`がこの案件のbare repositoryで
/// あることは、呼び出し側が確かめておく。
pub(super) fn send_host_refs(
    host: &dyn HostEnvironment,
    origin: &SandboxOrigin,
    sandbox: &str,
    git_dir: &str,
) -> Result<Vec<SentChange>> {
    let before = origin_refs(host, sandbox, git_dir)?;
    let refused = origin.refresh(host, sandbox, git_dir, None)?;
    let after = origin_refs(host, sandbox, git_dir)?;
    let mut changes = sent_changes(&before, &after);
    changes.extend(refused.into_iter().map(|refusal| SentChange::Refused {
        reference: refusal.reference,
        reason: refusal.reason,
    }));
    Ok(changes)
}

/// Sandboxのremote-tracking refとtagの、ref名から先端への対応。
fn origin_refs(
    host: &dyn HostEnvironment,
    sandbox: &str,
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
    let listed = sandbox::exec(host, sandbox, &args)?.require_success()?;
    Ok(listed
        .stdout_text()
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(reference, tip)| (reference.to_string(), tip.to_string()))
        .collect())
}

#[cfg(test)]
#[path = "send_host_refs_test.rs"]
mod send_host_refs_test;
