//! Sandbox内のGit worktree一覧。
//!
//! `git worktree list --porcelain -z`のNUL区切り出力だけを読み、表示textの検索や
//! 行の見た目に依存しない。bare rootの外を指すpathは案件の成果物として扱わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::error::{Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::project::SandboxLayout;

use super::sandbox;

/// `git worktree list --porcelain -z`の1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub bare: bool,
    pub detached: bool,
}

impl Entry {
    /// bare root配下へstandardizeできる場合の相対path。
    ///
    /// bare entry、およびbare rootの外を指すpathは、この案件のworktreeではない。
    pub fn relative_to(&self, bare_root: &str) -> Option<String> {
        if self.bare {
            return None;
        }
        let standardized = paths::lexically_standardize(Path::new(&self.path));
        let relative = standardized.strip_prefix(bare_root).ok()?;
        Some(paths::display(relative))
    }
}

/// Sandbox内のworktree一覧を読む。
///
/// 一覧を読めない場合は、worktreeが無いことと区別して外部commandの失敗とする。
pub fn list(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
) -> Result<Vec<Entry>> {
    let git_dir = layout.bare_git_dir();
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ],
    )?;
    let outcome = outcome.require_success()?;
    parse_list(&outcome.stdout_text())
}

/// NUL区切りのporcelain出力をparseする。
///
/// 空のfieldがrecordの区切りとなる。pathを持たないrecordは受け付けない。
pub fn parse_list(output: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut current: Option<Entry> = None;

    for field in output.split('\0') {
        if field.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        let (key, value) = match field.split_once(' ') {
            Some((key, value)) => (key, value),
            None => (field, ""),
        };
        match key {
            "worktree" => {
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                current = Some(Entry {
                    path: value.to_string(),
                    bare: false,
                    detached: false,
                });
            }
            "bare" | "detached" => match current.as_mut() {
                Some(entry) => {
                    if key == "bare" {
                        entry.bare = true;
                    } else {
                        entry.detached = true;
                    }
                }
                None => {
                    return Err(Error::new(
                        ErrorId::ExternalOutputUnparseable,
                        msg!(
                            "error-external-output-unparseable",
                            program = "git worktree list",
                            detail = format!("{key} appears before any worktree")
                        ),
                    ));
                }
            },
            _ => {}
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_porcelain_listing_is_read_field_by_field() {
        let output = "worktree /home/agent/work/repo\0bare\0\0worktree /home/agent/work/repo/repo.tree-0\0HEAD abc\0branch refs/heads/main\0\0worktree /home/agent/work/repo/repo.tree-1\0HEAD abc\0detached\0\0";
        let entries = parse_list(output).expect("the listing parses");
        assert_eq!(
            entries,
            vec![
                Entry {
                    path: "/home/agent/work/repo".to_string(),
                    bare: true,
                    detached: false,
                },
                Entry {
                    path: "/home/agent/work/repo/repo.tree-0".to_string(),
                    bare: false,
                    detached: false,
                },
                Entry {
                    path: "/home/agent/work/repo/repo.tree-1".to_string(),
                    bare: false,
                    detached: true,
                },
            ]
        );
        assert!(parse_list("").expect("an empty listing").is_empty());
        assert!(parse_list("detached\0\0").is_err());
    }

    #[test]
    fn only_paths_under_the_bare_root_are_this_projects_worktrees() {
        let root = "/home/agent/work/repo";
        let managed = Entry {
            path: format!("{root}/repo.tree-0"),
            bare: false,
            detached: false,
        };
        assert_eq!(managed.relative_to(root).as_deref(), Some("repo.tree-0"));

        let bare = Entry {
            path: root.to_string(),
            bare: true,
            detached: false,
        };
        assert_eq!(
            bare.relative_to(root),
            None,
            "a bare entry is not a worktree"
        );

        // `..`を含むpathは、standardizeした結果で判定する。
        let escaping = Entry {
            path: format!("{root}/../elsewhere"),
            bare: false,
            detached: false,
        };
        assert_eq!(escaping.relative_to(root), None);

        // 名前の前方一致だけでbare root配下とみなさない。
        let sibling = Entry {
            path: format!("{root}-other/tree"),
            bare: false,
            detached: false,
        };
        assert_eq!(sibling.relative_to(root), None);
    }
}
