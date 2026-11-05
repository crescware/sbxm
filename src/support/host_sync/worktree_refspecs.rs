use std::collections::BTreeSet;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Result, unparseable};
use crate::support::{sandbox, worktree};

/// 保存元にrefを書かず、NUL区切りの一覧から各HEADのfetch要求を作る。
pub(super) fn worktree_refspecs(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    git_dir: &str,
    incoming: &str,
) -> Result<Vec<String>> {
    let outcome = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ],
    )?;
    if !outcome.success() {
        return Err(crate::support::repository::sandbox_unreadable(
            sandbox_name,
            &outcome,
        ));
    }
    let output = std::str::from_utf8(&outcome.stdout)
        .map_err(|_| unparseable("git worktree list", "non-UTF-8 worktree listing"))?;
    refspecs(output, incoming)
}

fn refspecs(output: &str, incoming: &str) -> Result<Vec<String>> {
    let entries = worktree::parse_list(output)?;
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.bare {
            continue;
        }
        let oid = entry.head_oid()?;
        let basename = entry.path.rsplit('/').next().unwrap_or_default();
        let safe = !basename.is_empty()
            && !basename.starts_with('.')
            && !basename.ends_with('.')
            && !basename.as_bytes().ends_with(b".lock")
            && !basename.contains("..")
            && basename
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        let mut name = if safe {
            basename.to_string()
        } else {
            format!("worktree-{index}")
        };
        while !names.insert(name.clone()) {
            name = format!("worktree-{index}-{}", names.len());
        }
        result.push(format!("+{oid}:{incoming}worktrees/{name}"));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "worktree_refspecs_test.rs"]
mod worktree_refspecs_test;
