use crate::design::Cell;
use crate::msg;
use crate::support::protection::ConfirmableLoss;

/// 削除計画に示す、確認対象1件の説明。
pub fn confirmable_loss(loss: &ConfirmableLoss) -> Cell {
    let message = match loss {
        ConfirmableLoss::IgnoredPaths { worktree, paths } => msg!(
            "confirmable-loss-ignored-paths",
            worktree = worktree,
            count = paths.len()
        ),
        ConfirmableLoss::LocalRef {
            worktree,
            reference,
        } => msg!(
            "confirmable-loss-local-ref",
            worktree = worktree,
            reference = reference
        ),
        ConfirmableLoss::BranchUpstream {
            worktree,
            branch,
            upstream,
        } => msg!(
            "confirmable-loss-branch-upstream",
            worktree = worktree,
            branch = branch,
            upstream = upstream
        ),
        ConfirmableLoss::Tag { worktree, name } => {
            msg!("confirmable-loss-tag", worktree = worktree, name = name)
        }
        ConfirmableLoss::AdditionalRemote { worktree, name } => msg!(
            "confirmable-loss-additional-remote",
            worktree = worktree,
            name = name
        ),
        ConfirmableLoss::ReflogOnlyCommits { worktree, count } => msg!(
            "confirmable-loss-reflog-only-commits",
            worktree = worktree,
            count = count
        ),
        ConfirmableLoss::UnmanagedWorktree { worktree } => {
            msg!("confirmable-loss-unmanaged-worktree", worktree = worktree)
        }
        ConfirmableLoss::SandboxWritableLayer => msg!("confirmable-loss-sandbox-writable-layer"),
    };
    Cell::label(message)
}
