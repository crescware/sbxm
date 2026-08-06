use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use super::UnobservableReason;

/// 観測済みの拒否理由。1件でもあれば、確認を求めずrebuild/destroyを拒否する。
///
/// `ConfirmableLoss`と違い、ここに挙げる原因は削除計画にも明示確認にも進めない。
/// 変更や未pushのcommitのように、確認しても安全に削除できるとは言えないためである。
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
    /// refresh済みoriginのどのrefからもcommitへ到達できない。
    OriginUnreachable { reference: String, commit: String },
    /// originを権威ある状態として観測できない。
    OriginUnobservable {
        reference: String,
        commit: String,
        reason: UnobservableReason,
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
            ProtectionBlocker::OriginUnreachable { reference, commit } => Diagnostic::new(
                ErrorId::OriginCommitUnreachable,
                msg!(
                    "error-origin-commit-unreachable",
                    reference = reference,
                    commit = commit
                ),
            )
            .remediation(msg!("remediation-origin-commit-unreachable")),
            ProtectionBlocker::OriginUnobservable {
                reference,
                commit,
                reason,
            } => origin_unobservable_diagnostic(reference, commit, *reason),
        }
    }
}

fn origin_unobservable_diagnostic(
    reference: &str,
    commit: &str,
    reason: UnobservableReason,
) -> Diagnostic {
    match reason {
        UnobservableReason::OriginMissing => Diagnostic::new(
            ErrorId::OriginMissing,
            msg!("error-origin-missing", reference = reference),
        )
        .remediation(msg!("remediation-origin-missing")),
        UnobservableReason::RefreshFailed => Diagnostic::new(
            ErrorId::OriginRefreshFailed,
            msg!(
                "error-origin-refresh-failed",
                reference = reference,
                commit = commit
            ),
        )
        .remediation(msg!("remediation-origin-refresh-failed")),
        UnobservableReason::AdvertisementInvalid => Diagnostic::new(
            ErrorId::OriginAdvertisementInvalid,
            msg!(
                "error-origin-advertisement-invalid",
                reference = reference,
                commit = commit
            ),
        )
        .remediation(msg!("remediation-origin-advertisement-invalid")),
        UnobservableReason::ObjectMissing => Diagnostic::new(
            ErrorId::OriginObjectMissing,
            msg!(
                "error-origin-object-missing",
                reference = reference,
                commit = commit
            ),
        )
        .remediation(msg!("remediation-origin-object-missing")),
    }
}
