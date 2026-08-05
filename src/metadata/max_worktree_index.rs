use super::max_worktrees::MAX_WORKTREES;
use super::maximum_worktree_index::maximum_worktree_index;

/// 設定上限のworktree数をzero-based indexの上限へ変換した値。
pub const MAX_WORKTREE_INDEX: u32 = maximum_worktree_index(MAX_WORKTREES);
