use std::collections::BTreeSet;

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

use crate::support::sandbox;
use crate::support::worktree;

use super::{
    Assessment, Blocker, DestructiveOperation, Kind, Mode, OriginRecoveryFailure, Remote, Request,
    WorktreeReport,
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

/// 観測結果と、観測不能であることを区別する。後者もblockerとして保持するため、
/// `Result`の`Err`でassessment全体を捨てない。
enum Observed<T> {
    Yes(T),
    No,
}

/// worktree、Git操作、origin回収可能性を固定順序で評価する。
///
/// 観測そのものに失敗した場合も、観測不能のdiagnosticをblockerとして`Assessment`へ
/// 収める。独立して実行できる後続の検査は続け、`gate::authorize`が既知の全拒否理由を
/// 一度に表示する。`gate::assess`だけがこの関数を呼ぶ。
pub fn inspect(host: &dyn HostEnvironment, request: &Request<'_>) -> Assessment {
    let layout = request.layout;
    let sandbox_name = request.sandbox.as_str();
    let bare_root = layout.bare_root();
    let project = request.metadata.display_id();

    // 共有repositoryのないSandboxは、この案件の作業を1つも持たない。構築が途中で
    // 終わったSandboxがこれにあたり、worktreeが観測できないことを失うものがある
    // 徴候として読まない。
    let repository_exists = match sandbox::path_exists(host, sandbox_name, &layout.bare_git_dir()) {
        Ok(exists) => exists,
        Err(error) => {
            return Assessment::new(
                project,
                Vec::new(),
                vec![observation_blocker(&reclassify(
                    &error,
                    ErrorId::WorktreeInventoryUnobservable,
                    msg!("error-worktree-inventory-unobservable"),
                    status_remediation(
                        request.metadata.display_id().as_str(),
                        msg!("remediation-worktree-inventory-unobservable"),
                    ),
                    Fact::sandbox(sandbox_name),
                ))],
            );
        }
    };
    if !repository_exists {
        return Assessment::new(project, Vec::new(), Vec::new());
    }

    let entries = match worktree::list(host, sandbox_name, layout) {
        Ok(entries) => entries,
        Err(error) => {
            return Assessment::new(
                project,
                Vec::new(),
                vec![observation_blocker(&reclassify(
                    &error,
                    ErrorId::WorktreeInventoryUnobservable,
                    msg!("error-worktree-inventory-unobservable"),
                    status_remediation(
                        request.metadata.display_id().as_str(),
                        msg!("remediation-worktree-inventory-unobservable"),
                    ),
                    Fact::sandbox(sandbox_name),
                ))],
            );
        }
    };

    if !entries
        .iter()
        .any(|entry| entry.bare && entry.path == bare_root)
    {
        return Assessment::new(
            project,
            Vec::new(),
            vec![observation_blocker(&inventory_unobservable(
                request.metadata.display_id().as_str(),
                sandbox_name,
                "the worktree inventory did not contain the shared bare repository",
            ))],
        );
    }

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
            // bare root外のworktreeは、案件の成果物として扱えない。relativeが無いため
            // 他の検査は行えないが、拒否そのものは他のblockerと同列に集める。
            blockers.push(Blocker::WorktreeOutsideRepository {
                path: entry.path.clone(),
                root: bare_root.clone(),
            });
            continue;
        };
        let managed = declared.contains(&relative);

        // rebuildは同じ配置を再作成できないため、管理外の存在自体を拒否する。destroyは
        // 内容を他の検査と同列に確かめ、存在自体は`WorktreeReport::kind`が示す。
        if !managed && request.operation == DestructiveOperation::Rebuild {
            blockers.push(Blocker::UnmanagedWorktree {
                worktree: relative.clone(),
            });
        }

        let Some(report) = examine(
            host,
            sandbox_name,
            &entry,
            &relative,
            &project,
            managed,
            &mut blockers,
        ) else {
            continue;
        };
        worktrees.push(report);
    }

    Assessment::new(project, worktrees, blockers)
}

/// 1件のworktreeを検査し、既知のblockerまたは観測不能を集めながら結果を組み立てる。
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &worktree::Entry,
    relative: &str,
    project: &str,
    managed: bool,
    blockers: &mut Vec<Blocker>,
) -> Option<WorktreeReport> {
    let path = entry.path.as_str();

    check_tree_status(host, sandbox_name, path, relative, project, blockers);
    check_operation_in_progress(host, sandbox_name, path, relative, project, blockers);

    let head = match sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "HEAD"],
    ) {
        Ok(head) => Some(head),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, relative,
            )));
            None
        }
    };
    let branch_outcome = match sandbox::exec(
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
    ) {
        Ok(outcome) => Some(outcome),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, relative,
            )));
            None
        }
    };

    // `symbolic-ref --quiet`はdetached HEADを`1`で示す。それ以外の終了statusは判定しない。
    let branch_outcome = branch_outcome?;
    let attached = match sandbox::inner_exit_code(&branch_outcome) {
        Some(0) => true,
        Some(1) => false,
        _ => {
            blockers.push(observation_blocker(&local_refs_unobservable(
                &branch_outcome,
                project,
                relative,
            )));
            return None;
        }
    };

    let Some(head) = head else {
        // branchの持ち方までは観測できても、commitを特定できないため、origin回収性の
        // blockerを組み立てず、このworktreeのreportだけを欠測として扱う。
        return None;
    };
    let (mode, branch, remote) = if attached {
        let branch = branch_outcome.stdout_text().trim().to_string();
        let pushed = check_pushed(host, sandbox_name, path, relative, project, blockers);
        let Observed::Yes(reason) = pushed else {
            return None;
        };
        if let Some(reason) = reason {
            blockers.push(Blocker::OriginRecoveryNotProven {
                reference: branch.clone(),
                commit: head.clone(),
                reason,
            });
        }
        (Mode::Attached, Some(branch), Remote::Pushed)
    } else {
        let reachable =
            check_reachable_from_origin(host, sandbox_name, path, relative, project, blockers);
        let Observed::Yes(reason) = reachable else {
            return None;
        };
        if let Some(reason) = reason {
            blockers.push(Blocker::OriginRecoveryNotProven {
                reference: "HEAD".to_string(),
                commit: head.clone(),
                reason,
            });
        }
        (Mode::Detached, None, Remote::Reachable)
    };

    Some(WorktreeReport {
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
    project: &str,
    blockers: &mut Vec<Blocker>,
) {
    let outcome = match run(
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
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            blockers.push(observation_blocker(&reclassify(
                &error,
                ErrorId::WorktreeStatusUnobservable,
                msg!("error-worktree-status-unobservable"),
                open_remediation(project, msg!("remediation-worktree-status-unobservable")),
                Fact::worktree(relative),
            )));
            return;
        }
    };

    let (tracked_changes, untracked) = match parse_status(&outcome.stdout_text()) {
        Ok(status) => status,
        Err(detail) => {
            blockers.push(observation_blocker(&status_unobservable(
                project, relative, &detail,
            )));
            return;
        }
    };
    if tracked_changes {
        blockers.push(Blocker::TrackedChanges {
            worktree: relative.to_string(),
        });
    }
    if !untracked.is_empty() {
        blockers.push(Blocker::UntrackedPaths {
            worktree: relative.to_string(),
            paths: untracked,
        });
    }
}

/// `git status --porcelain=v2 -z`の出力を、追跡対象の変更の有無と未追跡pathへ分ける。
///
/// rename/copy entry（種別`2`）だけが2つ目のNUL区切りfield（原path）を持つ。Gitが
/// 成功を返しても、未知または不完全なrecordはcleanとして扱わず、その原因となった
/// fragmentを`Err`で返す。`--ignored`を渡していないため、ignore済みpathを示す
/// 種別`!`のrecordは正常な出力に現れない。現れた場合も既知の種別として素通りさせず、
/// 未知recordと同様に拒否する。
fn parse_status(output: &str) -> std::result::Result<(bool, Vec<String>), String> {
    if output.is_empty() {
        return Ok((false, Vec::new()));
    }
    if !output.ends_with('\0') {
        return Err(bounded(output));
    }

    let mut fields: Vec<&str> = output.split('\0').collect();
    fields.pop();
    if fields.is_empty() || fields.iter().any(|field| field.is_empty()) {
        return Err(bounded(output));
    }

    let mut tracked_changes = false;
    let mut untracked = Vec::new();
    let mut fields = fields.into_iter();

    while let Some(field) = fields.next() {
        let Some((kind, rest)) = field.split_once(' ') else {
            return Err(bounded(field));
        };
        match kind {
            "1" if valid_status_record(rest, 8) => tracked_changes = true,
            "u" if valid_status_record(rest, 10) => tracked_changes = true,
            "2" => {
                if !valid_status_record(rest, 9) {
                    return Err(bounded(field));
                }
                tracked_changes = true;
                let Some(original_path) = fields.next() else {
                    return Err(bounded(field));
                };
                if original_path.is_empty() {
                    return Err(bounded(field));
                }
            }
            "?" => {
                if rest.is_empty() {
                    return Err(bounded(field));
                }
                untracked.push(rest.to_string());
            }
            _ => return Err(bounded(field)),
        }
    }
    Ok((tracked_changes, untracked))
}

/// 診断へ載せる長さを抑える。observedな出力は攻撃者が選べるfile名を含みうる。
fn bounded(detail: &str) -> String {
    const MAX_CHARS: usize = 200;
    let mut truncated: String = detail.chars().take(MAX_CHARS).collect();
    if detail.chars().count() > MAX_CHARS {
        truncated.push('…');
    }
    truncated
}

/// 固定長のstatus recordを検証する。最後のfieldはpathなので、そこだけ空白を含められる。
fn valid_status_record(rest: &str, fields: usize) -> bool {
    let values: Vec<&str> = rest.splitn(fields, ' ').collect();
    values.len() == fields && values.iter().all(|value| !value.is_empty())
}

/// merge、rebase、cherry-pickのような操作が途中で止まっていないことを確かめる。
fn check_operation_in_progress(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    project: &str,
    blockers: &mut Vec<Blocker>,
) {
    let git_dir = match sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "--git-dir"],
    ) {
        Ok(git_dir) => git_dir,
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_git_operation(
                &error, project, relative,
            )));
            return;
        }
    };

    for marker in IN_PROGRESS_MARKERS {
        let candidate = format!("{git_dir}/{marker}");
        let probe = match sandbox::exec(host, sandbox_name, &["test", "-e", &candidate]) {
            Ok(probe) => probe,
            Err(error) => {
                blockers.push(observation_blocker(&reclassify_git_operation(
                    &error, project, relative,
                )));
                continue;
            }
        };
        // `test`はfileの不在を`1`で示す。commandを起動できなかったことを不在として読まない。
        match sandbox::inner_exit_code(&probe) {
            Some(0) => blockers.push(Blocker::GitOperationInProgress {
                worktree: relative.to_string(),
                operation: marker.to_string(),
            }),
            Some(1) => {}
            _ => {
                blockers.push(observation_blocker(&Error::single(
                    Diagnostic::new(
                        ErrorId::GitOperationUnobservable,
                        msg!("error-git-operation-unobservable"),
                    )
                    .fact(Fact::worktree(relative))
                    .fact(Fact::field(marker))
                    .remediation(open_remediation(
                        project,
                        msg!("remediation-git-operation-unobservable"),
                    ))
                    .external(probe.failure()),
                )));
            }
        }
    }
}

/// upstreamがあり、そこへ載っていないcommitを持たないことを確かめる。
///
/// 満たさない場合は理由を返す。観測できない場合もblockerへ記録する。
fn check_pushed(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    project: &str,
    blockers: &mut Vec<Blocker>,
) -> Observed<Option<OriginRecoveryFailure>> {
    let upstream = match sandbox::exec(
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
    ) {
        Ok(upstream) => upstream,
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, relative,
            )));
            return Observed::No;
        }
    };

    // upstream未設定はgitが非ゼロで示す。commandを起動できなかったことと区別する。
    match sandbox::inner_exit_code(&upstream) {
        Some(0) => {}
        Some(_) => return Observed::Yes(Some(OriginRecoveryFailure::NoUpstream)),
        None => {
            blockers.push(observation_blocker(&local_refs_unobservable(
                &upstream, project, relative,
            )));
            return Observed::No;
        }
    }
    let upstream = upstream.stdout_text().trim().to_string();
    let ahead = match run(
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
    ) {
        Ok(ahead) => ahead.stdout_text(),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, relative,
            )));
            return Observed::No;
        }
    };
    let count: u64 = if let Ok(count) = ahead.trim().parse() {
        count
    } else {
        blockers.push(observation_blocker(&local_refs_unparseable(
            project, relative, &ahead,
        )));
        return Observed::No;
    };
    if count == 0 {
        return Observed::Yes(None);
    }
    Observed::Yes(Some(OriginRecoveryFailure::AheadOfUpstream {
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
    project: &str,
    blockers: &mut Vec<Blocker>,
) -> Observed<Option<OriginRecoveryFailure>> {
    let unreachable = match run(
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
    ) {
        Ok(unreachable) => unreachable.stdout_text(),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, relative,
            )));
            return Observed::No;
        }
    };
    let count: u64 = if let Ok(count) = unreachable.trim().parse() {
        count
    } else {
        blockers.push(observation_blocker(&local_refs_unparseable(
            project,
            relative,
            &unreachable,
        )));
        return Observed::No;
    };
    if count == 0 {
        return Observed::Yes(None);
    }
    Observed::Yes(Some(OriginRecoveryFailure::UnreachableFromOrigin))
}

/// commandを実行し、非ゼロ終了を共通のerrorへ写像する。
fn run(host: &dyn HostEnvironment, sandbox_name: &str, args: &[&str]) -> Result<CommandOutcome> {
    sandbox::exec(host, sandbox_name, args)?.require_success()
}

/// 検査段階の失敗を、その段階固有のErrorIdへ翻訳する。
///
/// 元のdiagnosticが持つfactとexternal causeは、原因の説明として保持する。
fn reclassify(
    error: &Error,
    id: ErrorId,
    description: Msg,
    remediation: Remediation,
    fact: Fact,
) -> Error {
    let mut diagnostic = Diagnostic::new(id, description).remediation(remediation);
    if let Some(source) = error.diagnostics().first() {
        diagnostic.facts.clone_from(&source.facts);
        diagnostic.external.clone_from(&source.external);
    }
    diagnostic.facts.push(fact);
    Error::single(diagnostic)
}

/// Git directoryまたは進行中操作のmarkerの観測が失敗した場合の共通の写像。
fn reclassify_git_operation(error: &Error, project: &str, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::GitOperationUnobservable,
        msg!("error-git-operation-unobservable"),
        open_remediation(project, msg!("remediation-git-operation-unobservable")),
        Fact::worktree(relative),
    )
}

/// HEAD、branch、upstream、到達可能性の観測が失敗した場合の共通の写像。
fn reclassify_local_refs(error: &Error, project: &str, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::LocalRefsUnobservable,
        msg!("error-local-refs-unobservable"),
        open_remediation(project, msg!("remediation-local-refs-unobservable")),
        Fact::worktree(relative),
    )
}

/// commandは起動できたが、終了statusが判定対象の2値のどちらでもない場合。
fn local_refs_unobservable(outcome: &CommandOutcome, project: &str, relative: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRefsUnobservable,
            msg!("error-local-refs-unobservable"),
        )
        .fact(Fact::worktree(relative))
        .remediation(open_remediation(
            project,
            msg!("remediation-local-refs-unobservable"),
        ))
        .external(outcome.failure()),
    )
}

/// commandは成功したが、出力を数値として解釈できない場合。
fn local_refs_unparseable(project: &str, relative: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRefsUnobservable,
            msg!("error-local-refs-unobservable"),
        )
        .fact(Fact::worktree(relative))
        .fact(Fact::cause(detail))
        .remediation(open_remediation(
            project,
            msg!("remediation-local-refs-unobservable"),
        )),
    )
}

fn status_unobservable(project: &str, relative: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::WorktreeStatusUnobservable,
            msg!("error-worktree-status-unobservable"),
        )
        .fact(Fact::worktree(relative))
        .fact(Fact::cause(detail))
        .remediation(open_remediation(
            project,
            msg!("remediation-worktree-status-unobservable"),
        )),
    )
}

fn inventory_unobservable(project: &str, sandbox: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::WorktreeInventoryUnobservable,
            msg!("error-worktree-inventory-unobservable"),
        )
        .fact(Fact::sandbox(sandbox))
        .fact(Fact::cause(detail))
        .remediation(status_remediation(
            project,
            msg!("remediation-worktree-inventory-unobservable"),
        )),
    )
}

fn observation_blocker(error: &Error) -> Blocker {
    let diagnostic = match error.diagnostics().first() {
        Some(diagnostic) => diagnostic.clone(),
        None => Diagnostic::new(
            ErrorId::WorktreeInventoryUnobservable,
            msg!("error-worktree-inventory-unobservable"),
        ),
    };
    Blocker::unobservable(diagnostic)
}

fn open_remediation(project: &str, explanation: Msg) -> Remediation {
    Remediation::text(explanation).try_run(format!("sbxm open {project}"))
}

fn status_remediation(project: &str, explanation: Msg) -> Remediation {
    Remediation::text(explanation).try_run(format!("sbxm status {project}"))
}
