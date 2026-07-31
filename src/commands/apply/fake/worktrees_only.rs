use crate::commands::apply::Scope;

/// worktreeだけを適用するscope。
pub const WORKTREES_ONLY: Scope = Scope {
    files: false,
    worktrees: Some(3),
};
