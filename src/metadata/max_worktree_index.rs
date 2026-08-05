use super::last_worktree_index::last_worktree_index;
use super::max_worktrees::MAX_WORKTREES;

/// 設定上限のworktree数をzero-based indexの上限へ変換した値。
///
/// 案件ごとの上限は[`last_worktree_index`]で求める。こちらは案件を問わない天井であり、
/// metadataを読む前のpromptが受け付ける範囲に使う。
pub const MAX_WORKTREE_INDEX: u32 = last_worktree_index(MAX_WORKTREES);
