/// managed worktree数に対応する、zero-based indexの最後の値を返す。
///
/// worktree数の下限は[`MIN_WORKTREES`]なので0本は起こらない。到達しない入力へ
/// 専用のerrorを設けず、飽和させて0を返す。
///
/// [`MIN_WORKTREES`]: super::MIN_WORKTREES
pub const fn last_worktree_index(worktrees: u32) -> u32 {
    worktrees.saturating_sub(1)
}
