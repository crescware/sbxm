//! `Sandbox内のGit` worktree一覧。
//!
//! `git worktree list --porcelain -z`のNUL区切り出力だけを読み、表示textの検索や
//! 行の見た目に依存しない。bare rootの外を指すpathは案件の成果物として扱わない。

mod entry;
mod list;
mod parse_list;

pub use entry::Entry;
pub use list::list;
pub use parse_list::parse_list;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;
