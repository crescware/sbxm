use std::collections::{BTreeSet, HashSet};

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

use crate::support::sandbox;
use crate::support::worktree;

use super::{
    CommitCandidate, ConfirmableLoss, DestructiveOperation, Kind, Mode, OriginObservation,
    ProtectionAssessment, ProtectionBlocker, ProtectionRequest, Reachability, WorktreeReport,
    observe_for_mutation,
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

/// ローカル所有refを列挙するnamespace。`refs/remotes/*`はoriginの写しであり、
/// 対象に含めない。
const LOCAL_REF_NAMESPACES: [&str; 4] = ["refs/heads/", "refs/tags/", "refs/notes/", "refs/stash"];

/// worktree、Git操作、確認すれば削除してよい対象を固定順序で評価し、origin回収可能性
/// だけは全worktree分をまとめて1回observeする。
///
/// 観測そのものに失敗した場合は`Err`とする。既知のblockerと確認対象は打ち切らずに集め、
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
        return Ok(ProtectionAssessment::empty(
            request.operation,
            request.sandbox.clone(),
        ));
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

    let mut pending_worktrees = Vec::new();
    let mut blockers = Vec::new();
    let mut confirmable_losses = Vec::new();
    let mut candidates: Vec<CommitCandidate> = Vec::new();

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
        // 内容を他の検査と同列に確かめ、存在自体は`WorktreeReport::kind`と
        // `ConfirmableLoss::UnmanagedWorktree`が示す。
        if !managed {
            if request.operation == DestructiveOperation::Rebuild {
                blockers.push(ProtectionBlocker::UnmanagedWorktree {
                    worktree: relative.clone(),
                });
            } else {
                confirmable_losses.push(ConfirmableLoss::UnmanagedWorktree {
                    worktree: relative.clone(),
                });
            }
        }

        let pending = examine(
            host,
            sandbox_name,
            &entry,
            &relative,
            managed,
            Collected {
                blockers: &mut blockers,
                confirmable_losses: &mut confirmable_losses,
                candidates: &mut candidates,
            },
        )?;
        pending_worktrees.push(pending);
    }

    // sandboxの書き込み層は、削除するたびに必ず失われる。存在の有無を観測する対象では
    // なく、Sandboxがある限り常に成り立つ確認対象として1件だけ載せる。
    confirmable_losses.push(ConfirmableLoss::SandboxWritableLayer);

    let observation = observe_for_mutation(host, request.sandbox, layout, &candidates)?;
    let worktrees = finalize_worktrees(
        pending_worktrees,
        &observation,
        &mut blockers,
        &mut confirmable_losses,
    );

    Ok(ProtectionAssessment::new(
        request.operation,
        request.sandbox.clone(),
        worktrees,
        blockers,
        confirmable_losses,
        Some(observation),
    ))
}

/// origin観測に基づき、まだ確定していないworktree 1件の情報。
struct PendingWorktree {
    relative: String,
    kind: Kind,
    mode: Mode,
    head: String,
    branch: Option<String>,
    /// HEAD、またはcheckout中のbranchを指すcandidate。
    primary: CommitCandidate,
    /// HEAD以外のローカル所有ref。到達可能なら確認対象、そうでなければ拒否理由になる。
    local_refs: Vec<PendingLocalRef>,
}

/// origin観測に基づき、まだ確定していないローカル所有ref 1件の情報。
struct PendingLocalRef {
    candidate: CommitCandidate,
    /// 到達可能だった場合に載せる確認対象。
    loss: ConfirmableLoss,
    /// 到達可能だった場合に、branchのupstream追跡も合わせて確認対象へ載せる。
    branch_upstream: Option<ConfirmableLoss>,
}

/// `examine`が集める、既知のblocker・確認対象・origin candidateの集約先。
struct Collected<'a> {
    blockers: &'a mut Vec<ProtectionBlocker>,
    confirmable_losses: &'a mut Vec<ConfirmableLoss>,
    candidates: &'a mut Vec<CommitCandidate>,
}

/// 全worktree分のorigin観測が揃ってから、各worktreeの`Reachability`と、それに伴う
/// blocker・確認対象を確定する。
fn finalize_worktrees(
    pending_worktrees: Vec<PendingWorktree>,
    observation: &OriginObservation,
    blockers: &mut Vec<ProtectionBlocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) -> Vec<WorktreeReport> {
    let mut worktrees = Vec::with_capacity(pending_worktrees.len());
    for pending in pending_worktrees {
        let reachability = Reachability::classify(&pending.primary, observation);
        if let Some(blocker) = origin_blocker(&pending.primary, &reachability) {
            blockers.push(blocker);
        }
        for local_ref in pending.local_refs {
            let candidate_reachability = Reachability::classify(&local_ref.candidate, observation);
            if let Some(blocker) = origin_blocker(&local_ref.candidate, &candidate_reachability) {
                blockers.push(blocker);
            } else {
                confirmable_losses.push(local_ref.loss);
                if let Some(branch_upstream) = local_ref.branch_upstream {
                    confirmable_losses.push(branch_upstream);
                }
            }
        }
        worktrees.push(WorktreeReport {
            relative: pending.relative,
            kind: pending.kind,
            mode: pending.mode,
            head: pending.head,
            branch: pending.branch,
            reachability,
        });
    }
    worktrees
}

/// 分類結果を、拒否理由へ変換する。安全な結果（`Pushed`/`Reachable`）は`None`とする。
fn origin_blocker(
    candidate: &CommitCandidate,
    reachability: &Reachability,
) -> Option<ProtectionBlocker> {
    match reachability {
        Reachability::Unreachable => Some(ProtectionBlocker::OriginUnreachable {
            reference: candidate.reference().to_string(),
            commit: candidate.commit().to_string(),
        }),
        Reachability::Unobservable { reason } => Some(ProtectionBlocker::OriginUnobservable {
            reference: candidate.reference().to_string(),
            commit: candidate.commit().to_string(),
            reason: *reason,
        }),
        Reachability::Pushed { .. } | Reachability::Reachable { .. } => None,
    }
}

/// 1件のworktreeを検査し、既知のblockerと確認対象を集めながら観測結果を組み立てる。
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &worktree::Entry,
    relative: &str,
    managed: bool,
    collected: Collected<'_>,
) -> Result<PendingWorktree> {
    let Collected {
        blockers,
        confirmable_losses,
        candidates,
    } = collected;
    let path = entry.path.as_str();

    check_tree_status(host, sandbox_name, path, relative, blockers)?;
    collect_ignored_paths(host, sandbox_name, path, relative, confirmable_losses)?;
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

    let (mode, branch, primary) = if attached {
        let branch = branch_outcome.stdout_text().trim().to_string();
        let upstream = read_upstream(host, sandbox_name, path, relative)?;
        let candidate =
            CommitCandidate::new(format!("refs/heads/{branch}"), head.clone(), upstream);
        (Mode::Attached, Some(branch), candidate)
    } else {
        let candidate = CommitCandidate::new("HEAD".to_string(), head.clone(), None);
        (Mode::Detached, None, candidate)
    };
    candidates.push(primary.clone());

    let local_refs = collect_local_refs(host, sandbox_name, path, relative, branch.as_deref())?;
    for local_ref in &local_refs {
        candidates.push(local_ref.candidate.clone());
    }
    collect_additional_remotes(host, sandbox_name, path, relative, confirmable_losses)?;
    collect_reflog_only_commits(host, sandbox_name, path, relative, confirmable_losses)?;

    Ok(PendingWorktree {
        relative: relative.to_string(),
        kind: if managed {
            Kind::Managed
        } else {
            Kind::Unmanaged
        },
        mode,
        head,
        branch,
        primary,
        local_refs,
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

/// Gitが追跡しない無視対象のpathを集める。`check_tree_status`とは別commandにし、
/// この収集だけの失敗を`IgnoredPathsUnobservable`で個別に報告できるようにする。
fn collect_ignored_paths(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
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
            "--ignored=matching",
        ],
    )
    .map_err(|error| {
        reclassify(
            &error,
            ErrorId::IgnoredPathsUnobservable,
            msg!("error-ignored-paths-unobservable", worktree = relative),
            msg!("remediation-ignored-paths-unobservable"),
        )
    })?;

    let ignored: Vec<String> = outcome
        .stdout_text()
        .split('\0')
        .filter_map(|field| field.strip_prefix("! ").map(str::to_string))
        .collect();
    if !ignored.is_empty() {
        confirmable_losses.push(ConfirmableLoss::IgnoredPaths {
            worktree: relative.to_string(),
            paths: ignored,
        });
    }
    Ok(())
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

/// checkout中のbranchのupstream追跡先を読む。完全なref名で返す。
///
/// upstream未設定はgitが非ゼロで示す。commandを起動できなかったことと区別する。
fn read_upstream(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<Option<String>> {
    let outcome = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "rev-parse",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?;

    match sandbox::inner_exit_code(&outcome) {
        Some(0) => Ok(Some(outcome.stdout_text().trim().to_string())),
        Some(_) => Ok(None),
        None => Err(local_refs_unobservable(&outcome, relative)),
    }
}

/// HEAD以外のローカル所有ref（branch、tag、notes、stash）を集める。
///
/// 現在checkout中のbranchは、project metadataの start refから再現できるため対象外と
/// する。到達可能かどうかは、全worktree分の観測をまとめたあとで判定する。
fn collect_local_refs(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    current_branch: Option<&str>,
) -> Result<Vec<PendingLocalRef>> {
    let outcome = run(
        host,
        sandbox_name,
        &[
            "git",
            "-C",
            path,
            "for-each-ref",
            "--format=%(refname)%09%(objectname)%09%(upstream)",
            LOCAL_REF_NAMESPACES[0],
            LOCAL_REF_NAMESPACES[1],
            LOCAL_REF_NAMESPACES[2],
            LOCAL_REF_NAMESPACES[3],
        ],
    )
    .map_err(|error| reclassify_local_refs(&error, relative))?;

    let mut pending = Vec::new();
    for line in outcome.stdout_text().lines() {
        let mut fields = line.split('\t');
        let Some(reference) = fields.next().filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some(commit) = fields.next().filter(|value| !value.is_empty()) else {
            continue;
        };
        let upstream = fields.next().filter(|value| !value.is_empty());

        let branch_name = reference.strip_prefix("refs/heads/");
        if branch_name.is_some() && branch_name == current_branch {
            continue;
        }

        let loss = if let Some(name) = reference.strip_prefix("refs/tags/") {
            ConfirmableLoss::Tag {
                worktree: relative.to_string(),
                name: name.to_string(),
            }
        } else {
            ConfirmableLoss::LocalRef {
                worktree: relative.to_string(),
                reference: reference.to_string(),
            }
        };
        let branch_upstream = match (branch_name, upstream) {
            (Some(branch), Some(upstream)) => Some(ConfirmableLoss::BranchUpstream {
                worktree: relative.to_string(),
                branch: branch.to_string(),
                upstream: upstream.to_string(),
            }),
            _ => None,
        };

        pending.push(PendingLocalRef {
            candidate: CommitCandidate::new(
                reference.to_string(),
                commit.to_string(),
                upstream.map(str::to_string),
            ),
            loss,
            branch_upstream,
        });
    }
    Ok(pending)
}

/// originとは別の、追加のremote名を集める。remote URLは読まない。
fn collect_additional_remotes(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) -> Result<()> {
    let outcome = run(host, sandbox_name, &["git", "-C", path, "remote"]).map_err(|error| {
        reclassify(
            &error,
            ErrorId::RemoteConfigurationUnobservable,
            msg!(
                "error-remote-configuration-unobservable",
                worktree = relative
            ),
            msg!("remediation-remote-configuration-unobservable"),
        )
    })?;

    for name in outcome.stdout_text().lines() {
        let name = name.trim();
        if name.is_empty() || name == "origin" {
            continue;
        }
        confirmable_losses.push(ConfirmableLoss::AdditionalRemote {
            worktree: relative.to_string(),
            name: name.to_string(),
        });
    }
    Ok(())
}

/// どの参照からも到達できないが、reflogにだけ残るcommitを数える。
fn collect_reflog_only_commits(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) -> Result<()> {
    let reflog = run(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-list", "--walk-reflogs", "--all"],
    )
    .map_err(|error| reclassify_reflog(&error, relative))?
    .stdout_text();
    let live = run(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-list", "--all"],
    )
    .map_err(|error| reclassify_reflog(&error, relative))?
    .stdout_text();

    let live: HashSet<&str> = live.lines().collect();
    let count = reflog
        .lines()
        .filter(|commit| !live.contains(commit))
        .count();
    if count > 0 {
        confirmable_losses.push(ConfirmableLoss::ReflogOnlyCommits {
            worktree: relative.to_string(),
            count: u64::try_from(count).unwrap_or(u64::MAX),
        });
    }
    Ok(())
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

/// HEAD、branch、upstream、ローカルrefの観測が失敗した場合の共通の写像。
fn reclassify_local_refs(error: &Error, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::LocalRefsUnobservable,
        msg!("error-local-refs-unobservable", worktree = relative),
        msg!("remediation-local-refs-unobservable"),
    )
}

/// reflogの観測が失敗した場合の共通の写像。
fn reclassify_reflog(error: &Error, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::ReflogUnobservable,
        msg!("error-reflog-unobservable", worktree = relative),
        msg!("remediation-reflog-unobservable"),
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
