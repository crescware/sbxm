use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use super::OriginRecoveryFailure;

/// 層Aの観測済み拒否理由。
///
/// variantごとに固有の`ErrorId`へ一対一で変換する。原因ごとに異なるIDを出すため、
/// 共通の`UnsavedWork`のようなIDへまとめない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionBlocker {
    /// 追跡対象ファイルに未コミットの変更がある。
    TrackedChanges { worktree: String },
    /// 無視対象ではない未追跡ファイルがある。
    UntrackedPaths {
        worktree: String,
        paths: Vec<String>,
    },
    /// merge、rebaseのようなGit操作が途中で止まっている。
    GitOperationInProgress { worktree: String, operation: String },
    /// rebuild対象に、metadataから配置を再現できない作業ツリーがある。
    UnmanagedWorktree { worktree: String },
    /// commitをoriginから回収できると証明できない。
    OriginRecoveryNotProven {
        reference: String,
        commit: String,
        reason: OriginRecoveryFailure,
    },
}

impl ProtectionBlocker {
    /// 利用者へ表示する診断へ変換する。
    pub(super) fn diagnostic(&self) -> Diagnostic {
        match self {
            ProtectionBlocker::TrackedChanges { worktree } => Diagnostic::new(
                ErrorId::WorktreeTrackedChanges,
                msg!("error-worktree-tracked-changes", worktree = worktree),
            )
            .remediation(msg!("remediation-worktree-tracked-changes")),
            ProtectionBlocker::UntrackedPaths { worktree, paths } => Diagnostic::new(
                ErrorId::WorktreeUntrackedPaths,
                msg!(
                    "error-worktree-untracked-paths",
                    worktree = worktree,
                    count = paths.len()
                ),
            )
            .fact(Fact::paths(paths))
            .remediation(msg!("remediation-worktree-untracked-paths")),
            ProtectionBlocker::GitOperationInProgress {
                worktree,
                operation,
            } => Diagnostic::new(
                ErrorId::GitOperationInProgress,
                msg!(
                    "error-git-operation-in-progress",
                    worktree = worktree,
                    operation = operation
                ),
            )
            .remediation(msg!("remediation-git-operation-in-progress")),
            ProtectionBlocker::UnmanagedWorktree { worktree } => Diagnostic::new(
                ErrorId::UnmanagedWorktreePresent,
                msg!("error-unmanaged-worktree-present", path = worktree),
            )
            .remediation(msg!("remediation-unmanaged-worktree-present")),
            ProtectionBlocker::OriginRecoveryNotProven {
                reference,
                commit,
                reason,
            } => origin_recovery_diagnostic(reference, commit, reason),
        }
    }
}

fn origin_recovery_diagnostic(
    reference: &str,
    commit: &str,
    reason: &OriginRecoveryFailure,
) -> Diagnostic {
    match reason {
        OriginRecoveryFailure::NoUpstream => Diagnostic::new(
            ErrorId::OriginUpstreamMissing,
            msg!("error-origin-upstream-missing", reference = reference),
        )
        .remediation(msg!("remediation-origin-upstream-missing")),
        OriginRecoveryFailure::AheadOfUpstream { upstream, count } => Diagnostic::new(
            ErrorId::OriginCommitUnpushed,
            msg!(
                "error-origin-commit-unpushed",
                reference = reference,
                upstream = upstream,
                count = count
            ),
        )
        .remediation(msg!("remediation-origin-commit-unpushed")),
        OriginRecoveryFailure::UnreachableFromOrigin => Diagnostic::new(
            ErrorId::OriginCommitUnreachable,
            msg!(
                "error-origin-commit-unreachable",
                reference = reference,
                commit = commit
            ),
        )
        .remediation(msg!("remediation-origin-commit-unreachable")),
    }
}
