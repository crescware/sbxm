use std::collections::BTreeSet;

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

use crate::support::sandbox;
use crate::support::worktree;

use super::{
    DestructiveOperation, Kind, Mode, OriginRecoveryFailure, ProtectionAssessment,
    ProtectionBlocker, ProtectionRequest, Remote, WorktreeReport,
};

/// 進行中のGit操作を示すfile。1つでもあれば削除しない。
const IN_PROGRESS_MARKERS: [&str; 6] = [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "BISECT_LOG",
    "rebase-merge",
    "rebase-apply",
];

/// worktree、Git操作、origin回収可能性を固定順序で評価する。
///
/// 観測そのものに失敗した場合は`Err`とする。既知のblockerは打ち切らずに集め、
/// `ProtectionAssessment`へ収める。`gate::assess`だけがこの関数を呼ぶ。
pub fn inspect(
    host: &dyn HostEnvironment,
    request: &ProtectionRequest<'_>,
) -> Result<ProtectionAssessment> {
    let layout = request.layout;
    let sandbox_name = request.sandbox.as_str();
    let bare_root = layout.bare_root();

    // 共有repositoryのないSandboxは、この案件の作業を1つも持たない。構築が途中で
    // 終わったSandboxがこれにあたり、worktreeが観測できないことを失うものがある
    // 徴候として読まない。
    if !sandbox::path_exists(host, sandbox_name, &layout.bare_git_dir())? {
        return Ok(ProtectionAssessment::new(Vec::new(), Vec::new()));
    }

    let entries = worktree::list(host, sandbox_name, layout).map_err(|error| {
        reclassify(
            &error,
            ErrorId::WorktreeInventoryUnobservable,
            msg!(
                "error-worktree-inventory-unobservable",
                sandbox = sandbox_name
            ),
            msg!("remediation-worktree-inventory-unobservable"),
        )
    })?;

    let declared: BTreeSet<String> = layout
        .worktree_names(request.metadata.provisioning.requested_worktrees)
        .into_iter()
        .collect();

    let mut worktrees = Vec::new();
    let mut blockers = Vec::new();

    for entry in entries {
        if entry.bare {
            continue;
        }
        let Some(relative) = entry.relative_to(&bare_root) else {
            // bare root外のworktreeは、案件の成果物として扱えない。保存状態とは別の
            // 拒否であり、他のblockerと同列には集めない。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::WorktreeOutsideRepository,
                    msg!(
                        "error-worktree-outside-repository",
                        path = entry.path,
                        root = bare_root
                    ),
                )
                .remediation(msg!("remediation-worktree-outside-repository")),
            ));
        };
        let managed = declared.contains(&relative);

        // rebuildは同じ配置を再作成できないため、管理外の存在自体を拒否する。destroyは
        // 内容を他の検査と同列に確かめ、存在自体は`WorktreeReport::kind`が示す。
        if !managed && request.operation == DestructiveOperation::Rebuild {
            blockers.push(ProtectionBlocker::UnmanagedWorktree {
                worktree: relative.clone(),
            });
        }

        let report = examine(
            host,
            sandbox_name,
            &entry,
            &relative,
            managed,
            &mut blockers,
        )?;
        worktrees.push(report);
    }

    Ok(ProtectionAssessment::new(worktrees, blockers))
}

/// 1件のworktreeを検査し、既知のblockerを集めながら観測結果を組み立てる。
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &worktree::Entry,
    relative: &str,
    managed: bool,
    blockers: &mut Vec<ProtectionBlocker>,
) -> Result<WorktreeReport> {
    let path = entry.path.as_str();

    check_tree_status(host, sandbox_name, path, relative, blockers)?;
    check_operation_in_progress(host, sandbox_name, path, relative, blockers)?;

    let head = sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "HEAD"],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?;
    let branch_outcome = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "symbolic-ref",
            "--quiet",
            "--short",
            "HEAD",
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?;

    // `symbolic-ref --quiet`はdetached HEADを`1`で示す。それ以外の終了statusは判定しない。
    let attached = match sandbox::inner_exit_code(&branch_outcome) {
        Some(0) => true,
        Some(1) => false,
        _ => return Err(local_refs_unobservable(&branch_outcome, relative)),
    };

    let (mode, branch, remote) = if attached {
        let branch = branch_outcome.stdout_text().trim().to_string();
        if let Some(reason) = check_pushed(host, sandbox_name, path, relative)? {
            blockers.push(ProtectionBlocker::OriginRecoveryNotProven {
                reference: branch.clone(),
                commit: head.clone(),
                reason,
            });
        }
        (Mode::Attached, Some(branch), Remote::Pushed)
    } else {
        if let Some(reason) = check_reachable_from_origin(host, sandbox_name, path, relative)? {
            blockers.push(ProtectionBlocker::OriginRecoveryNotProven {
                reference: "HEAD".to_string(),
                commit: head.clone(),
                reason,
            });
        }
        (Mode::Detached, None, Remote::Reachable)
    };

    Ok(WorktreeReport {
        relative: relative.to_string(),
        kind: if managed {
            Kind::Managed
        } else {
            Kind::Unmanaged
        },
        mode,
        head,
        branch,
        remote,
    })
}

/// 追跡対象の変更と未追跡pathを分けて集める。
fn check_tree_status(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    blockers: &mut Vec<ProtectionBlocker>,
) -> Result<()> {
    let outcome = run(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
        ],
    )
    .map_err(|error| {
        reclassify(
            &error,
            ErrorId::WorktreeStatusUnobservable,
            msg!("error-worktree-status-unobservable", worktree = relative),
            msg!("remediation-worktree-status-unobservable"),
        )
    })?;

    let (tracked_changes, untracked) = parse_status(&outcome.stdout_text());
    if tracked_changes {
        blockers.push(ProtectionBlocker::TrackedChanges {
            worktree: relative.to_string(),
        });
    }
    if !untracked.is_empty() {
        blockers.push(ProtectionBlocker::UntrackedPaths {
            worktree: relative.to_string(),
            paths: untracked,
        });
    }
    Ok(())
}

/// `git status --porcelain=v2 -z`の出力を、追跡対象の変更の有無と未追跡pathへ分ける。
///
/// rename/copy entry（種別`2`）だけが2つ目のNUL区切りfield（原path）を持つ。
fn parse_status(output: &str) -> (bool, Vec<String>) {
    let mut tracked_changes = false;
    let mut untracked = Vec::new();
    let mut fields = output.split('\0').filter(|field| !field.is_empty());

    while let Some(field) = fields.next() {
        let kind = field.split(' ').next().unwrap_or("");
        match kind {
            "1" | "u" => tracked_changes = true,
            "2" => {
                tracked_changes = true;
                fields.next(); // 原path。値としては使わない。
            }
            "?" => {
                if let Some(path) = field.strip_prefix("? ") {
                    untracked.push(path.to_string());
                }
            }
            _ => {}
        }
    }
    (tracked_changes, untracked)
}

/// merge、rebase、cherry-pickのような操作が途中で止まっていないことを確かめる。
fn check_operation_in_progress(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    blockers: &mut Vec<ProtectionBlocker>,
) -> Result<()> {
    let git_dir = sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "--git-dir"],
    )
    .map_err(|error| reclassify_git_operation(&error, relative))?;

    for marker in IN_PROGRESS_MARKERS {
        let candidate = format!("{git_dir}/{marker}");
        let probe = sandbox::exec(host, sandbox_name, &["test", "-e", &candidate])
            .map_err(|error| reclassify_git_operation(&error, relative))?;
        // `test`はfileの不在を`1`で示す。commandを起動できなかったことを不在として読まない。
        match sandbox::inner_exit_code(&probe) {
            Some(0) => blockers.push(ProtectionBlocker::GitOperationInProgress {
                worktree: relative.to_string(),
                operation: marker.to_string(),
            }),
            Some(1) => {}
            _ => {
                return Err(Error::single(
                    Diagnostic::new(
                        ErrorId::GitOperationUnobservable,
                        msg!("error-git-operation-unobservable", worktree = relative),
                    )
                    .fact(Fact::field(marker))
                    .remediation(msg!("remediation-git-operation-unobservable"))
                    .external(probe.failure()),
                ));
            }
        }
    }
    Ok(())
}

/// upstreamがあり、そこへ載っていないcommitを持たないことを確かめる。
///
/// 満たさない場合は理由を返す。観測できない場合だけ`Err`とする。
fn check_pushed(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<Option<OriginRecoveryFailure>> {
    let upstream = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?;

    // upstream未設定はgitが非ゼロで示す。commandを起動できなかったことと区別する。
    match sandbox::inner_exit_code(&upstream) {
        Some(0) => {}
        Some(_) => return Ok(Some(OriginRecoveryFailure::NoUpstream)),
        None => return Err(local_refs_unobservable(&upstream, relative)),
    }
    let upstream = upstream.stdout_text().trim().to_string();
    let ahead = run(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "rev-list",
            "--count",
            &format!("{upstream}..HEAD"),
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?
    .stdout_text();
    let count: u64 = ahead
        .trim()
        .parse()
        .map_err(|_| local_refs_unparseable(relative, &ahead))?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(OriginRecoveryFailure::AheadOfUpstream {
        upstream,
        count,
    }))
}

/// detached HEADが、originのいずれかのrefから到達できることを確かめる。
fn check_reachable_from_origin(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<Option<OriginRecoveryFailure>> {
    let unreachable = run(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "rev-list",
            "--count",
            "HEAD",
            "--not",
            "--remotes=origin",
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?
    .stdout_text();
    let count: u64 = unreachable
        .trim()
        .parse()
        .map_err(|_| local_refs_unparseable(relative, &unreachable))?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(OriginRecoveryFailure::UnreachableFromOrigin))
}

/// commandを実行し、非ゼロ終了を共通のerrorへ写像する。
fn run(host: &dyn HostEnvironment, sandbox_name: &str, args: &[&str]) -> Result<CommandOutcome> {
    sandbox::exec(host, sandbox_name, args)?.require_success()
}

/// 検査段階の失敗を、その段階固有のErrorIdへ翻訳する。
///
/// 元のdiagnosticが持つfactとexternal causeは、原因の説明として保持する。
fn reclassify(error: &Error, id: ErrorId, description: Msg, remediation: Msg) -> Error {
    let mut diagnostic = Diagnostic::new(id, description).remediation(remediation);
    if let Some(source) = error.diagnostics().first() {
        diagnostic.facts.clone_from(&source.facts);
        diagnostic.external.clone_from(&source.external);
    }
    Error::single(diagnostic)
}

/// Git directoryまたは進行中操作のmarkerの観測が失敗した場合の共通の写像。
fn reclassify_git_operation(error: &Error, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::GitOperationUnobservable,
        msg!("error-git-operation-unobservable", worktree = relative),
        msg!("remediation-git-operation-unobservable"),
    )
}

/// HEAD、branch、upstream、到達可能性の観測が失敗した場合の共通の写像。
fn reclassify_local_refs(error: &Error, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::LocalRefsUnobservable,
        msg!("error-local-refs-unobservable", worktree = relative),
        msg!("remediation-local-refs-unobservable"),
    )
}

/// commandは起動できたが、終了statusが判定対象の2値のどちらでもない場合。
fn local_refs_unobservable(outcome: &CommandOutcome, relative: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRefsUnobservable,
            msg!("error-local-refs-unobservable", worktree = relative),
        )
        .remediation(msg!("remediation-local-refs-unobservable"))
        .external(outcome.failure()),
    )
}

/// commandは成功したが、出力を数値として解釈できない場合。
fn local_refs_unparseable(relative: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRefsUnobservable,
            msg!("error-local-refs-unobservable", worktree = relative),
        )
        .fact(Fact::cause(detail))
        .remediation(msg!("remediation-local-refs-unobservable")),
    )
}
