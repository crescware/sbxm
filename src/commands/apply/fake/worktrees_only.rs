use crate::commands::apply::Scope;

/// worktreeだけを適用するscope。
pub const WORKTREES_ONLY: Scope = Scope {
    files: false,
    force: false,
    worktrees: Some(3),
};
