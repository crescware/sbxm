//! managed worktreeの作成と、既存treeの引き受け。

mod adopt_worktree;
mod create_worktree;
mod ensure_worktrees;
mod mode_for;
mod provision_worktree;
mod verify_mode;

pub(super) use adopt_worktree::adopt_worktree;
pub(super) use create_worktree::create_worktree;
pub use ensure_worktrees::ensure_worktrees;
pub(super) use mode_for::mode_for;
pub(super) use provision_worktree::provision_worktree;
pub(super) use verify_mode::verify_mode;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;
