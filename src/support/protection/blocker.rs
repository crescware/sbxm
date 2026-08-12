use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use super::OriginRecoveryFailure;

/// 表示するpathの上限。これを超える件数は総数だけ`Fact::count`で示す。
///
/// 未追跡pathは`node_modules`のような大量生成物を含みうる。上限を置かないと、1件の
/// diagnosticが数万行になる。
const MAX_LISTED_PATHS: usize = 20;

/// 利用者へ確認を求めずに削除を拒否する、観測済みの危険状態または観測不能。
///
/// 観測済みの原因ごとに異なる`ErrorId`を出し、観測不能の場合は検査段階が作った
/// diagnosticをそのまま保持する。共通の`UnsavedWork`のようなIDへまとめない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocker {
    /// 必要な観測が成立せず、安全を証明できない。
    Unobservable { diagnostic: Diagnostic },
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
    /// worktreeが共有bare repositoryの外を指す。
    WorktreeOutsideRepository { path: String, root: String },
    /// commitをoriginから回収できると証明できない。
    OriginRecoveryNotProven {
        reference: String,
        commit: String,
        reason: OriginRecoveryFailure,
    },
}

impl Blocker {
    /// 利用者へ表示する診断へ変換する。
    pub(super) fn diagnostic(&self, project: &str) -> Diagnostic {
        match self {
            Blocker::Unobservable { diagnostic } => diagnostic.clone(),
            Blocker::TrackedChanges { worktree } => Diagnostic::new(
                ErrorId::WorktreeTrackedChanges,
                msg!("error-worktree-tracked-changes"),
            )
            .fact(Fact::worktree(worktree))
            .remediation(open(project, msg!("remediation-worktree-tracked-changes"))),
            Blocker::UntrackedPaths { worktree, paths } => {
                untracked_paths_diagnostic(project, worktree, paths)
            }
            Blocker::GitOperationInProgress {
                worktree,
                operation,
            } => Diagnostic::new(
                ErrorId::GitOperationInProgress,
                msg!("error-git-operation-in-progress"),
            )
            .fact(Fact::worktree(worktree))
            .fact(Fact::operation(operation))
            .remediation(open(project, msg!("remediation-git-operation-in-progress"))),
            Blocker::UnmanagedWorktree { worktree } => Diagnostic::new(
                ErrorId::UnmanagedWorktreePresent,
                msg!("error-unmanaged-worktree-present"),
            )
            .fact(Fact::worktree(worktree))
            .remediation(status(
                project,
                msg!("remediation-unmanaged-worktree-present"),
            )),
            Blocker::WorktreeOutsideRepository { path, root } => Diagnostic::new(
                ErrorId::WorktreeOutsideRepository,
                msg!("error-worktree-outside-repository"),
            )
            .fact(Fact::path(path))
            .fact(Fact::root(root))
            .remediation(status(
                project,
                msg!("remediation-worktree-outside-repository"),
            )),
            Blocker::OriginRecoveryNotProven {
                reference,
                commit,
                reason,
            } => origin_recovery_diagnostic(project, reference, commit, reason),
        }
    }

    /// 観測不能の診断を、他のblockerと同じ安定順序で保持する。
    pub(super) fn unobservable(diagnostic: Diagnostic) -> Blocker {
        Blocker::Unobservable { diagnostic }
    }
}

/// 未追跡pathの一覧を`MAX_LISTED_PATHS`件までに絞り、総数を別のFactで示す。
fn untracked_paths_diagnostic(project: &str, worktree: &str, paths: &[String]) -> Diagnostic {
    let shown = &paths[..paths.len().min(MAX_LISTED_PATHS)];
    let mut diagnostic = Diagnostic::new(
        ErrorId::WorktreeUntrackedPaths,
        msg!("error-worktree-untracked-paths"),
    )
    .fact(Fact::worktree(worktree))
    .fact(Fact::paths(shown));
    if paths.len() > MAX_LISTED_PATHS {
        diagnostic = diagnostic.fact(Fact::count(paths.len()));
    }
    diagnostic.remediation(open(project, msg!("remediation-worktree-untracked-paths")))
}

fn open(project: &str, explanation: crate::diagnostics::Msg) -> Remediation {
    Remediation::text(explanation).try_run(format!("sbxm open {project}"))
}

fn status(project: &str, explanation: crate::diagnostics::Msg) -> Remediation {
    Remediation::text(explanation).try_run(format!("sbxm status {project}"))
}

fn origin_recovery_diagnostic(
    project: &str,
    reference: &str,
    commit: &str,
    reason: &OriginRecoveryFailure,
) -> Diagnostic {
    match reason {
        OriginRecoveryFailure::NoUpstream => Diagnostic::new(
            ErrorId::OriginUpstreamMissing,
            msg!("error-origin-upstream-missing"),
        )
        .fact(Fact::reference(reference))
        .fact(Fact::commit(commit))
        .remediation(open(project, msg!("remediation-origin-upstream-missing"))),
        OriginRecoveryFailure::AheadOfUpstream { upstream, count } => Diagnostic::new(
            ErrorId::OriginCommitUnpushed,
            msg!("error-origin-commit-unpushed"),
        )
        .fact(Fact::reference(reference))
        .fact(Fact::commit(commit))
        .fact(Fact::upstream(upstream))
        .fact(Fact::count(*count))
        .remediation(open(project, msg!("remediation-origin-commit-unpushed"))),
        OriginRecoveryFailure::UnreachableFromOrigin => Diagnostic::new(
            ErrorId::OriginCommitUnreachable,
            msg!("error-origin-commit-unreachable"),
        )
        .fact(Fact::reference(reference))
        .fact(Fact::commit(commit))
        .remediation(open(project, msg!("remediation-origin-commit-unreachable"))),
    }
}
