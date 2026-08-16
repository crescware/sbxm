use std::collections::BTreeMap;

use crate::command::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::protection::{self, CommitCandidate, Reachability, UnobservableReason};
use crate::support::sandbox;
use crate::support::worktree;

use crate::commands::status::project::{ProjectStatus, Value, WorktreeRow};

use super::worktree_state;

/// Sandbox内のworktreeを、metadataと突き合わせて分類する。
///
/// 作業状態とRemoteの回収可能性は別の軸である。前者はworktreeごとに読み、後者は
/// 全worktreeのcandidateをまとめて一度だけread-only観測へ渡す。read-only観測はfetchを
/// 行わないため、手元のremote objectが不足している場合も`unobservable`のまま表示する。
pub fn check_worktrees(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let entries = match worktree::list(host, name.as_str(), layout) {
        Ok(entries) => entries,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            status.push("status-item-worktrees", Value::NotObserved);
            return;
        }
    };

    let project = status.project.clone();
    let (pending, value) =
        collect_pending_worktrees(host, name, layout, metadata, entries, &project, status);
    let observation = observe_candidates(host, name, layout, &pending, status);
    append_worktree_rows(&project, pending, observation.as_ref(), status);
    status.push("status-item-worktrees", value);
}

struct PendingWorktree {
    path: String,
    kind: &'static str,
    mode: Value,
    state: Value,
    candidate: Option<CommitCandidate>,
}

fn collect_pending_worktrees(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    entries: Vec<worktree::Entry>,
    project: &str,
    status: &mut ProjectStatus,
) -> (Vec<PendingWorktree>, Value) {
    let bare_root = layout.bare_root();
    let declared = layout.worktree_names(metadata.provisioning.requested_worktrees);
    let mut seen = Vec::new();
    let mut pending = Vec::new();
    let mut value = Value::Ready;

    for entry in entries {
        if entry.bare {
            continue;
        }
        let Some(worktree) =
            read_pending_worktree(host, name, &entry, &bare_root, &declared, project, status)
        else {
            value = Value::Mismatch;
            continue;
        };
        if worktree.state == Value::Mismatch {
            value = Value::Mismatch;
        } else if worktree.state == Value::NotObserved && value == Value::Ready {
            value = Value::NotObserved;
        }
        seen.push(worktree.path.clone());
        pending.push(worktree);
    }

    append_absent_worktrees(&declared, &seen, &mut pending, &mut value, status);
    (pending, value)
}

fn read_pending_worktree(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    entry: &worktree::Entry,
    bare_root: &str,
    declared: &[String],
    project: &str,
    status: &mut ProjectStatus,
) -> Option<PendingWorktree> {
    let Some(relative) = entry.relative_to(bare_root) else {
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!("error-sandbox-repository-unusable"),
            )
            .fact(Fact::path(&entry.path))
            .fact(Fact::reason(msg!(
                "cause-worktree-outside-shared-repository"
            ))),
        );
        return None;
    };
    let managed = declared.contains(&relative);
    let mode = if entry.detached {
        Value::Detached
    } else {
        Value::Attached
    };
    let state = worktree_state(host, name, &entry.path, status);
    let candidate = read_candidate(
        host,
        name,
        &entry.path,
        &relative,
        entry.detached,
        project,
        status,
    );
    Some(PendingWorktree {
        path: relative,
        kind: if managed { "managed" } else { "unmanaged" },
        mode,
        state,
        candidate,
    })
}

fn append_absent_worktrees(
    declared: &[String],
    seen: &[String],
    pending: &mut Vec<PendingWorktree>,
    value: &mut Value,
    status: &mut ProjectStatus,
) {
    for name in declared {
        if seen.contains(name) {
            continue;
        }
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!("error-sandbox-repository-unusable"),
            )
            .fact(Fact::path(name))
            .fact(Fact::reason(msg!("cause-managed-worktree-absent"))),
        );
        pending.push(PendingWorktree {
            path: name.clone(),
            kind: "managed",
            mode: Value::Mismatch,
            state: Value::Mismatch,
            candidate: None,
        });
        *value = Value::Mismatch;
    }
}

fn observe_candidates(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    pending: &[PendingWorktree],
    status: &mut ProjectStatus,
) -> Option<protection::OriginObservation> {
    let candidates: Vec<CommitCandidate> = pending
        .iter()
        .filter_map(|worktree| worktree.candidate.clone())
        .collect();
    if candidates.is_empty() {
        return None;
    }
    match protection::observe_read_only(host, name, layout, &candidates) {
        Ok(observation) => Some(observation),
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

/// worktreeごとのRemoteを分類し、原因ごとに1件へ畳んだ観測不能診断をまとめて積む。
///
/// `observation`が`None`（`observe_read_only`自体が起動できなかった）場合、その原因は
/// `observe_candidates`が既にdiagnosticとして積んでいる。ここで同じ原因を
/// `ReadOnlyDataInsufficient`として再度診断すると、誤った理由の診断がworktreeの数だけ
/// 重複する。`observation`が`Some`のときだけ、`Reachability::classify`が返す実際の理由で
/// 集約する。
fn append_worktree_rows(
    project: &str,
    pending: Vec<PendingWorktree>,
    observation: Option<&protection::OriginObservation>,
    status: &mut ProjectStatus,
) {
    let classified: Vec<(PendingWorktree, Reachability)> = pending
        .into_iter()
        .map(|worktree| {
            let remote = classify_worktree(worktree.candidate.as_ref(), observation);
            (worktree, remote)
        })
        .collect();

    if observation.is_some() {
        let mut unobservable_by_reason: BTreeMap<UnobservableReason, Vec<String>> = BTreeMap::new();
        for (worktree, remote) in &classified {
            if let (Some(candidate), Reachability::Unobservable { reason }) =
                (worktree.candidate.as_ref(), remote)
            {
                unobservable_by_reason
                    .entry(*reason)
                    .or_default()
                    .push(candidate.reference().to_string());
            }
        }
        for (reason, references) in unobservable_by_reason {
            status.diagnostics.push(
                protection::Blocker::diagnostic_for_unobservable_reachability(
                    project,
                    &references,
                    reason,
                ),
            );
        }
    }

    for (worktree, remote) in classified {
        status.worktrees.push(WorktreeRow {
            path: worktree.path,
            kind: worktree.kind,
            mode: worktree.mode,
            state: worktree.state,
            remote,
        });
    }
}

/// `state`列とは違い、`Reachability::Unreachable`は観測に成功した通常の状態であり、
/// diagnosticにしない。`candidate`が無いworktreeと、observationそのものが得られなかった
/// worktreeは、どちらも同じ`ReadOnlyDataInsufficient`として表示する。
fn classify_worktree(
    candidate: Option<&CommitCandidate>,
    observation: Option<&protection::OriginObservation>,
) -> Reachability {
    let Some(candidate) = candidate else {
        return read_only_unobservable();
    };
    observation.map_or_else(read_only_unobservable, |observation| {
        Reachability::classify(candidate, observation)
    })
}

/// statusがRemoteを分類するために必要なcandidateを、worktreeの状態とは別に読む。
fn read_candidate(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    path: &str,
    relative: &str,
    detached: bool,
    project: &str,
    status: &mut ProjectStatus,
) -> Option<CommitCandidate> {
    let head = match sandbox::read(
        host,
        name.as_str(),
        &["git", "-C", path, "rev-parse", "HEAD"],
    ) {
        Ok(head) if !head.is_empty() => head,
        Ok(_) => {
            read_only_candidate_failure(name, relative, project, status);
            return None;
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            return None;
        }
    };

    if detached {
        return Some(CommitCandidate::new("HEAD".to_string(), head, None));
    }

    let branch = match sandbox::read(
        host,
        name.as_str(),
        &[
            "git",
            "-C",
            path,
            "symbolic-ref",
            "--quiet",
            "--short",
            "HEAD",
        ],
    ) {
        Ok(branch) if !branch.is_empty() => branch,
        Ok(_) => {
            read_only_candidate_failure(name, relative, project, status);
            return None;
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            return None;
        }
    };

    let upstream = match sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "-C",
            path,
            "rev-parse",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    ) {
        Ok(outcome) => match sandbox::inner_exit_code(&outcome) {
            Some(0) => {
                let upstream = outcome.stdout_text().trim().to_string();
                if upstream.is_empty() {
                    read_only_candidate_failure(name, relative, project, status);
                    return None;
                }
                Some(upstream)
            }
            // No upstream is a meaningful candidate state; Reachable can still be proved from
            // another origin ref. Git uses more than one nonzero status for this lookup.
            Some(_) => None,
            _ => {
                status.diagnostics.extend(
                    sandbox::unobservable(&outcome, relative)
                        .diagnostics()
                        .iter()
                        .cloned(),
                );
                return None;
            }
        },
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            return None;
        }
    };

    Some(CommitCandidate::new(
        format!("refs/heads/{branch}"),
        head,
        upstream,
    ))
}

fn read_only_unobservable() -> Reachability {
    Reachability::Unobservable {
        reason: UnobservableReason::ReadOnlyDataInsufficient,
    }
}

fn read_only_candidate_failure(
    name: &SandboxName,
    relative: &str,
    project: &str,
    status: &mut ProjectStatus,
) {
    status.diagnostics.push(
        Diagnostic::new(
            ErrorId::OriginReadOnlyDataInsufficient,
            msg!("error-origin-read-only-data-insufficient"),
        )
        .fact(Fact::sandbox(name.as_str()))
        .fact(Fact::worktree(relative))
        .fact(Fact::reason(msg!(
            "cause-origin-read-only-data-insufficient"
        )))
        .remediation(
            Remediation::text(msg!("remediation-origin-read-only-data-insufficient"))
                .try_run(format!("sbxm open {project}")),
        ),
    );
}
