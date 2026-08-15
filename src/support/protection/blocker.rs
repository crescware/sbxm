use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use super::{CommitCandidate, Reachability, UnobservableReason};

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
    /// refresh済みoriginのどのrefからもcommitへ到達できない。
    OriginUnreachable { reference: String, commit: String },
    /// originを権威ある状態として観測できず、commitを回収できるか判定できない。
    ///
    /// 観測不能はrepositoryへの観測1回につき1つの原因であり、影響するreferenceの数だけ
    /// 同じ診断を繰り返さない。`references`は影響したcandidateのref名を1件へ畳む。
    OriginUnobservable {
        references: Vec<String>,
        reason: UnobservableReason,
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
            Blocker::OriginUnreachable { reference, commit } => Diagnostic::new(
                ErrorId::OriginCommitUnreachable,
                msg!("error-origin-commit-unreachable"),
            )
            .fact(Fact::reference(reference))
            .fact(Fact::commit(commit))
            .remediation(open(project, msg!("remediation-origin-commit-unreachable"))),
            Blocker::OriginUnobservable { references, reason } => {
                origin_unobservable_diagnostic(project, references, *reason)
            }
        }
    }

    /// `status` の `Remote` 列に対応する利用者向け診断を組み立てる。
    pub(crate) fn diagnostic_for_reachability(
        project: &str,
        sandbox: &str,
        candidate: &CommitCandidate,
        reachability: &Reachability,
    ) -> Option<Diagnostic> {
        let facts = |diagnostic: Diagnostic| {
            diagnostic
                .fact(Fact::sandbox(sandbox))
                .fact(Fact::reference(candidate.reference()))
                .fact(Fact::commit(candidate.commit()))
        };

        match reachability {
            Reachability::Pushed { .. } | Reachability::Reachable { .. } => None,
            Reachability::Unreachable => Some(facts(
                Diagnostic::new(
                    ErrorId::OriginCommitUnreachable,
                    msg!("error-origin-commit-unreachable"),
                )
                .remediation(open(project, msg!("remediation-origin-commit-unreachable"))),
            )),
            Reachability::Unobservable { reason } => {
                let diagnostic = match reason {
                    UnobservableReason::ReadOnlyDataInsufficient => Diagnostic::new(
                        ErrorId::OriginReadOnlyDataInsufficient,
                        msg!("error-origin-read-only-data-insufficient"),
                    )
                    .fact(Fact::reason(msg!(
                        "cause-origin-read-only-data-insufficient"
                    )))
                    .remediation(
                        Remediation::text(msg!("remediation-origin-read-only-data-insufficient"))
                            .try_run(format!("sbxm open {project}")),
                    ),
                    other => origin_unobservable_diagnostic(
                        project,
                        &[candidate.reference().to_string()],
                        *other,
                    ),
                };
                Some(facts(diagnostic))
            }
        }
    }

    /// 観測不能の診断を、他のblockerと同じ安定順序で保持する。
    pub(super) fn unobservable(diagnostic: Diagnostic) -> Blocker {
        Blocker::Unobservable { diagnostic }
    }

    /// fingerprintの入力に使う、翻訳しない安定表記。
    ///
    /// `Unobservable`は`ErrorId`だけを写す。保持する`Diagnostic`は外部commandのstderrと
    /// 表示用messageを含み、どちらもfingerprintの入力に含めないと決めているためである。
    /// 同じ`ErrorId`の観測不能が複数あっても件数は保たれ、blockerが1件でもあれば
    /// `require_no_blockers`が先に拒否するため、この粒度で足りる。
    pub(super) fn fingerprint_key(&self) -> String {
        match self {
            Blocker::Unobservable { diagnostic } => {
                format!("unobservable\u{1f}{}", diagnostic.id.as_str())
            }
            Blocker::TrackedChanges { worktree } => format!("tracked-changes\u{1f}{worktree}"),
            Blocker::UntrackedPaths { worktree, paths } => {
                let mut sorted = paths.clone();
                sorted.sort();
                format!(
                    "untracked-paths\u{1f}{worktree}\u{1f}{}",
                    sorted.join("\u{1e}")
                )
            }
            Blocker::GitOperationInProgress {
                worktree,
                operation,
            } => format!("git-operation-in-progress\u{1f}{worktree}\u{1f}{operation}"),
            Blocker::UnmanagedWorktree { worktree } => {
                format!("unmanaged-worktree\u{1f}{worktree}")
            }
            Blocker::WorktreeOutsideRepository { path, root } => {
                format!("worktree-outside-repository\u{1f}{path}\u{1f}{root}")
            }
            Blocker::OriginUnreachable { reference, commit } => {
                format!("origin-unreachable\u{1f}{reference}\u{1f}{commit}")
            }
            Blocker::OriginUnobservable { references, reason } => {
                let mut sorted = references.clone();
                sorted.sort();
                format!(
                    "origin-unobservable\u{1f}{}\u{1f}{}",
                    sorted.join("\u{1e}"),
                    reason.fingerprint_key()
                )
            }
        }
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

/// `sbxm status --global`と、この案件の`sbxm status`の両方を示す。
///
/// originのrefreshそのものが失敗した理由は、この案件のSandboxだけでなくhost側の
/// network/credentialにも及びうる。両方の切り分けを案内する。
fn status_global_and_project(project: &str, explanation: crate::diagnostics::Msg) -> Remediation {
    Remediation::text(explanation)
        .try_run("sbxm status --global")
        .try_run(format!("sbxm status {project}"))
}

/// 権威あるorigin観測が成立しなかった理由ごとに、固有の`ErrorId`を出す。
///
/// どの理由も「回収できると証明できていない」点は同じだが、利用者が次に直すものは
/// 理由ごとに違う。共通のIDへ丸めない。観測不能はrepositoryへの観測1回につき1つの
/// 原因なので、影響した`references`は1件のdiagnosticへ畳んで示す。
fn origin_unobservable_diagnostic(
    project: &str,
    references: &[String],
    reason: UnobservableReason,
) -> Diagnostic {
    let (id, description, remediation) = match reason {
        UnobservableReason::OriginMissing => (
            ErrorId::OriginMissing,
            msg!("error-origin-missing"),
            open(project, msg!("remediation-origin-missing")),
        ),
        UnobservableReason::RefreshFailed => (
            ErrorId::OriginRefreshFailed,
            msg!("error-origin-refresh-failed"),
            status_global_and_project(project, msg!("remediation-origin-refresh-failed")),
        ),
        UnobservableReason::AdvertisementInvalid => (
            ErrorId::OriginAdvertisementInvalid,
            msg!("error-origin-advertisement-invalid"),
            open(project, msg!("remediation-origin-advertisement-invalid")),
        ),
        UnobservableReason::ObjectMissing => (
            ErrorId::OriginObjectMissing,
            msg!("error-origin-object-missing"),
            open(project, msg!("remediation-origin-object-missing")),
        ),
        UnobservableReason::ReadOnlyDataInsufficient => (
            ErrorId::OriginReadOnlyDataInsufficient,
            msg!("error-origin-read-only-data-insufficient"),
            Remediation::text(msg!("remediation-origin-read-only-data-insufficient"))
                .try_run(format!("sbxm open {project}")),
        ),
    };
    let shown = &references[..references.len().min(MAX_LISTED_PATHS)];
    let mut diagnostic = Diagnostic::new(id, description).fact(Fact::references(shown));
    if references.len() > MAX_LISTED_PATHS {
        diagnostic = diagnostic.fact(Fact::count(references.len()));
    }
    diagnostic.remediation(remediation)
}
