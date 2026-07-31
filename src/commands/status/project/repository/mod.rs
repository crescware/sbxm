//! bare repositoryとmanaged worktreeの診断。

mod check_bare_repository;
mod check_worktrees;
mod worktree_state;

pub(super) use check_bare_repository::check_bare_repository;
pub(super) use check_worktrees::check_worktrees;
pub(super) use worktree_state::worktree_state;

#[cfg(test)]
#[path = "repository_test.rs"]
mod repository_test;
