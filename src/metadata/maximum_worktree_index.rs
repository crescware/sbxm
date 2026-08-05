const WORKTREE_INDEX_OFFSET: u32 = 1;

/// managed worktree数に対応するzero-based indexの最大値を返す。
pub const fn maximum_worktree_index(worktrees: u32) -> u32 {
    worktrees.saturating_sub(WORKTREE_INDEX_OFFSET)
}
