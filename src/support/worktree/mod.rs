//! Gitのworktree一覧。
//!
//! `git worktree list --porcelain -z`のNUL区切り出力だけを読み、表示textの検索や
//! 行の見た目に依存しない。`list`はSandbox内の一覧を読み、bare rootの外を指すpathは
//! 案件の成果物として扱わない。`parse_list`は、hostのrepositoryの一覧を読むのにも使う。

mod entry;
mod inspection;
mod inspection_commands;
mod list;
mod list_command;
mod parse_list;

pub use entry::Entry;
pub use inspection::Inspection;
pub use inspection_commands::inspection_commands;
pub use list::list;
pub use list_command::list_command;
pub use parse_list::parse_list;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;
