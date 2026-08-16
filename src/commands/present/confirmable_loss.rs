use crate::design::Cell;
use crate::msg;
use crate::support::protection::ConfirmableLoss;

/// 削除計画に示す、確認対象1件の説明。
pub fn confirmable_loss(loss: &ConfirmableLoss) -> Cell {
    let message = match loss {
        ConfirmableLoss::IgnoredPaths { worktree, paths } => msg!(
            "confirmable-loss-ignored-paths",
            worktree = worktree,
            count = paths.len(),
            paths = paths.join(", ")
        ),
        ConfirmableLoss::LocalRef { reference } => {
            msg!("confirmable-loss-local-ref", reference = reference)
        }
        ConfirmableLoss::BranchUpstream { branch, upstream } => msg!(
            "confirmable-loss-branch-upstream",
            branch = branch,
            upstream = upstream
        ),
        ConfirmableLoss::Tag { name } => msg!("confirmable-loss-tag", name = name),
        ConfirmableLoss::AdditionalRemote { name } => {
            msg!("confirmable-loss-additional-remote", name = name)
        }
        ConfirmableLoss::ReflogOnlyCommits { count } => {
            msg!("confirmable-loss-reflog-only-commits", count = count)
        }
        ConfirmableLoss::UnmanagedWorktree { worktree } => {
            msg!("confirmable-loss-unmanaged-worktree", worktree = worktree)
        }
        ConfirmableLoss::SandboxWritableLayer => msg!("confirmable-loss-sandbox-writable-layer"),
    };
    Cell::label(message)
}
