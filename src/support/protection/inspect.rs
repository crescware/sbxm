use std::collections::{BTreeSet, HashSet};

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths;

use crate::support::sandbox;
use crate::support::worktree;

use super::{
    Assessment, BARE_GIT_DIR_PROBE, Blocker, ConfirmableLoss, DestructiveOperation, Kind, Mode,
    OriginRecoveryFailure, Remote, Request, WorktreeReport, answered,
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

/// 観測結果と、観測不能であることを区別する。後者もblockerとして保持するため、
/// `Result`の`Err`でassessment全体を捨てない。
enum Observed<T> {
    Yes(T),
    No,
}

/// `git status --porcelain=v2`のignore済みrecordの扱い。
///
/// `--ignored`を渡していない実行では、種別`!`のrecordは正常な出力に現れない。現れた
/// 場合に既知の種別として素通りさせないよう、収集する実行と拒否する実行を呼び出し側が
/// 選ぶ。
enum IgnoredRecords {
    Reject,
    Collect,
}

/// `git status --porcelain=v2 -z`から読み取った、1つのworktreeの状態。
struct StatusReport {
    tracked_changes: bool,
    untracked: Vec<String>,
    ignored: Vec<String>,
}

/// worktree、Git操作、origin回収可能性、確認すれば削除してよい対象を固定順序で評価する。
///
/// 観測そのものに失敗した場合も、観測不能のdiagnosticをblockerとして`Assessment`へ
/// 収める。独立して実行できる後続の検査は続け、`gate::authorize`が既知の全拒否理由を
/// 一度に表示する。`gate::assess`だけがこの関数を呼ぶ。
///
/// worktree単位の観測（作業ツリーの状態、無視対象path、進行中のGit操作、HEAD）は
/// worktreeごとに行う。ref、remote、reflogは共有bare repositoryが持つため、worktree数に
/// かかわらず1回だけ観測する。どちらもSandboxの中を見るため、その前にmount元の
/// workspace directoryがhostに在ることを確かめる。
pub fn inspect(host: &dyn HostEnvironment, request: &Request<'_>) -> Assessment {
    let layout = request.layout;
    let sandbox_name = request.sandbox.as_str();
    let bare_root = layout.bare_root();
    let bare_git_dir = layout.bare_git_dir();
    let project = request.metadata.display_id();

    // Sandboxの書き込み層は、repositoryを観測できるかどうかにかかわらず、削除するたびに
    // 必ず失われる。存在の有無を観測する対象ではないため、この関数へ入った時点で1件だけ
    // 計上する。これが無いと「Sandboxがそもそも無い」観測と区別がつかない。
    let mut confirmable_losses = vec![ConfirmableLoss::SandboxWritableLayer];

    if let Some(blocker) = missing_workspace(request) {
        return observation_failure(request, project, confirmable_losses, Some(blocker));
    }

    // 共有repositoryのないSandboxは、この案件の作業を1つも持たない。構築が途中で
    // 終わったSandboxがこれにあたり、worktreeが観測できないことを失うものがある
    // 徴候として読まない。
    let repository_exists = match repository_present(host, sandbox_name, &bare_git_dir) {
        Ok(exists) => exists,
        Err(error) => {
            let blocker = worktree_inventory_unobservable(request, sandbox_name, &error);
            return observation_failure(request, project, confirmable_losses, Some(blocker));
        }
    };
    if !repository_exists {
        return observation_failure(request, project, confirmable_losses, None);
    }

    let entries = match worktree::list(host, sandbox_name, layout) {
        Ok(entries) => entries,
        Err(error) => {
            let blocker = worktree_inventory_unobservable(request, sandbox_name, &error);
            return observation_failure(request, project, confirmable_losses, Some(blocker));
        }
    };

    if let Some(blocker) = missing_bare_repository(&entries, &bare_root, request, sandbox_name) {
        return observation_failure(request, project, confirmable_losses, Some(blocker));
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
        // 内容を他の検査と同列に確かめ、存在自体は`WorktreeReport::kind`と
        // `ConfirmableLoss::UnmanagedWorktree`が示す。
        if !managed {
            if request.operation == DestructiveOperation::Rebuild {
                blockers.push(Blocker::UnmanagedWorktree {
                    worktree: relative.clone(),
                });
            } else {
                confirmable_losses.push(ConfirmableLoss::UnmanagedWorktree {
                    worktree: relative.clone(),
                });
            }
        }

        let Some(report) = examine(
            host,
            sandbox_name,
            &entry,
            &relative,
            &project,
            managed,
            &mut blockers,
            &mut confirmable_losses,
        ) else {
            continue;
        };
        worktrees.push(report);
    }

    // どこかのworktreeがcheckoutしているbranchは、project metadataのstart refから
    // 再現できるため、ref名の損失に数えない。worktreeごとに判定すると、他のworktreeが
    // checkoutしているbranchを損失として数えてしまう。
    let checked_out: BTreeSet<String> = worktrees
        .iter()
        .filter_map(|report| report.branch.clone())
        .collect();

    collect_repository_inventory(
        host,
        sandbox_name,
        &RepositoryScope {
            git_dir: &bare_git_dir,
            root: &bare_root,
            project: &project,
        },
        &checked_out,
        &mut blockers,
        &mut confirmable_losses,
    );

    Assessment::new(
        request.operation,
        project,
        request.sandbox.clone(),
        worktrees,
        blockers,
        confirmable_losses,
    )
}

/// mount元のworkspace directoryがhostに在ることを、Sandboxの中を見る前に確かめる。
///
/// mount元が無いSandboxへの`sbx exec`は、内側のcommandを起動できないまま終了status
/// だけを返す。その終了statusは、内側のcommandが答えた「不在」と区別できない
/// (詳細は`workspace_missing`)。`sbx exec`の答えへ頼る前に、hostを直接見る。
fn missing_workspace(request: &Request<'_>) -> Option<Blocker> {
    match sandbox::workspace_exists(request.workspace_root, request.sandbox) {
        Ok(true) => None,
        Ok(false) => Some(Blocker::unobservable(workspace_missing(request))),
        Err(error) => Some(observation_blocker(&error)),
    }
}

/// 共有bare repositoryがSandboxの中に在るかを観測する。
///
/// 直前のhost側確認とこの`sbx exec`の間にもworkspace directoryは消えうる。その場合
/// `sbx exec`は内側のshellを起動できないまま終了statusだけを返し、その値は`test -e`が
/// 答える`0`/`1`と重なるため、終了statusだけでは区別できない(詳細は
/// `BARE_GIT_DIR_PROBE`)。内側のshellが実際に走った場合だけstdoutへ書かれる印が
/// 無ければ、終了statusを`test`の答えとして読まない。
fn repository_present(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    bare_git_dir: &str,
) -> Result<bool> {
    let probe = sandbox::exec(
        host,
        sandbox_name,
        &["sh", "-c", BARE_GIT_DIR_PROBE, "sh", bare_git_dir],
    )?;
    if probe.stdout_text().is_empty() {
        return Err(sandbox::unobservable(&probe, bare_git_dir));
    }
    match answered(&probe, bare_git_dir)? {
        0 => Ok(true),
        1 => Ok(false),
        _ => Err(sandbox::unobservable(&probe, bare_git_dir)),
    }
}

/// worktree一覧に共有bare repositoryが含まれないなら、その観測不能を返す。
///
/// 一覧が共有repositoryを含まないなら、読んだのは別の何かである。worktreeが1件も
/// 無い状態として読まない。
fn missing_bare_repository(
    entries: &[worktree::Entry],
    bare_root: &str,
    request: &Request<'_>,
    sandbox_name: &str,
) -> Option<Blocker> {
    if entries
        .iter()
        .any(|entry| entry.bare && entry.path == bare_root)
    {
        return None;
    }
    Some(observation_blocker(&inventory_unobservable(
        request.metadata.display_id().as_str(),
        sandbox_name,
        "the worktree inventory did not contain the shared bare repository",
    )))
}

/// 共有bare repositoryに対して1回だけ行う観測の宛先。
struct RepositoryScope<'a> {
    git_dir: &'a str,
    root: &'a str,
    project: &'a str,
}

/// worktreeではなくrepositoryが持つinventoryを、まとめて1回だけ観測する。
///
/// ref、remote、reflogはlinked worktreeが共有する。worktreeごとに数えると、同じtagや
/// remoteを worktree の数だけ削除計画へ並べることになる。
fn collect_repository_inventory(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    repository: &RepositoryScope<'_>,
    checked_out: &BTreeSet<String>,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) {
    collect_local_refs(
        host,
        sandbox_name,
        repository,
        checked_out,
        blockers,
        confirmable_losses,
    );
    collect_additional_remotes(host, sandbox_name, repository, blockers, confirmable_losses);
    collect_reflog_only_commits(host, sandbox_name, repository, blockers, confirmable_losses);
}

/// 1件のworktreeを検査し、既知のblocker・確認対象・観測不能を集めながら結果を組み立てる。
#[allow(clippy::too_many_arguments)]
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &worktree::Entry,
    relative: &str,
    project: &str,
    managed: bool,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) -> Option<WorktreeReport> {
    let path = entry.path.as_str();

    check_tree_status(host, sandbox_name, path, relative, project, blockers);
    collect_ignored_paths(
        host,
        sandbox_name,
        path,
        relative,
        project,
        blockers,
        confirmable_losses,
    );
    check_operation_in_progress(host, sandbox_name, path, relative, project, blockers);

    let (head, mode, branch, remote) =
        resolve_position(host, sandbox_name, path, relative, project, blockers)?;

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

/// HEADが指すcommitと、branchへの接続状態を観測する。origin回収可能性の判定も行う。
///
/// commitを特定できない、または観測不能な場合は`None`を返し、呼び出し側はこの
/// worktreeのreportを欠測として扱う。
fn resolve_position(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    project: &str,
    blockers: &mut Vec<Blocker>,
) -> Option<(String, Mode, Option<String>, Remote)> {
    let head = match sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "HEAD"],
    ) {
        Ok(head) => Some(head),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error,
                project,
                Fact::worktree(relative),
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
                &error,
                project,
                Fact::worktree(relative),
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
                Fact::worktree(relative),
            )));
            return None;
        }
    };

    // branchの持ち方までは観測できても、commitを特定できないため、origin回収性の
    // blockerを組み立てず、このworktreeのreportだけを欠測として扱う。
    let head = head?;
    if attached {
        let branch = branch_outcome.stdout_text().trim().to_string();
        let Observed::Yes(reason) =
            check_pushed(host, sandbox_name, path, relative, project, blockers)
        else {
            return None;
        };
        if let Some(reason) = reason {
            blockers.push(Blocker::OriginRecoveryNotProven {
                reference: branch.clone(),
                commit: head.clone(),
                reason,
            });
        }
        Some((head, Mode::Attached, Some(branch), Remote::Pushed))
    } else {
        let reachable = reachable_from_origin(
            host,
            sandbox_name,
            &["git", "-C", path],
            "HEAD",
            project,
            Fact::worktree(relative),
            blockers,
        );
        let Observed::Yes(reachable) = reachable else {
            return None;
        };
        if !reachable {
            blockers.push(Blocker::OriginRecoveryNotProven {
                reference: "HEAD".to_string(),
                commit: head.clone(),
                reason: OriginRecoveryFailure::UnreachableFromOrigin,
            });
        }
        Some((head, Mode::Detached, None, Remote::Reachable))
    }
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

    let status = match parse_status(&outcome.stdout_text(), &IgnoredRecords::Reject) {
        Ok(status) => status,
        Err(detail) => {
            blockers.push(observation_blocker(&status_unobservable(
                project, relative, &detail,
            )));
            return;
        }
    };
    if status.tracked_changes {
        blockers.push(Blocker::TrackedChanges {
            worktree: relative.to_string(),
        });
    }
    if !status.untracked.is_empty() {
        blockers.push(Blocker::UntrackedPaths {
            worktree: relative.to_string(),
            paths: status.untracked,
        });
    }
}

/// `git status --porcelain=v2 -z`の出力を、追跡対象の変更・未追跡path・無視対象pathへ分ける。
///
/// rename/copy entry（種別`2`）だけが2つ目のNUL区切りfield（原path）を持つ。Gitが
/// 成功を返しても、未知または不完全なrecordはcleanとして扱わず、その原因となった
/// fragmentを`Err`で返す。ignore済みpathを示す種別`!`のrecordは、`--ignored`を渡した
/// 実行でだけ受け付ける。
fn parse_status(
    output: &str,
    ignored_records: &IgnoredRecords,
) -> std::result::Result<StatusReport, String> {
    let mut report = StatusReport {
        tracked_changes: false,
        untracked: Vec::new(),
        ignored: Vec::new(),
    };
    if output.is_empty() {
        return Ok(report);
    }
    if !output.ends_with('\0') {
        return Err(bounded(output));
    }

    let mut fields: Vec<&str> = output.split('\0').collect();
    fields.pop();
    if fields.is_empty() || fields.iter().any(|field| field.is_empty()) {
        return Err(bounded(output));
    }

    let mut fields = fields.into_iter();

    while let Some(field) = fields.next() {
        let Some((kind, rest)) = field.split_once(' ') else {
            return Err(bounded(field));
        };
        match kind {
            "1" if valid_status_record(rest, 8) => report.tracked_changes = true,
            "u" if valid_status_record(rest, 10) => report.tracked_changes = true,
            "2" => {
                if !valid_status_record(rest, 9) {
                    return Err(bounded(field));
                }
                report.tracked_changes = true;
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
                report.untracked.push(rest.to_string());
            }
            "!" if matches!(ignored_records, IgnoredRecords::Collect) => {
                if rest.is_empty() {
                    return Err(bounded(field));
                }
                report.ignored.push(rest.to_string());
            }
            _ => return Err(bounded(field)),
        }
    }
    Ok(report)
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

/// Gitが追跡しない無視対象のpathを集める。`check_tree_status`とは別commandにし、
/// この収集だけの失敗を`IgnoredPathsUnobservable`で個別に報告できるようにする。
///
/// `--untracked-files=all`は渡さない。渡すと`--ignored=traditional`が無視対象
/// directoryを1 fileずつ展開し、削除計画に出す件数が生成物の増減で揺れる。directory
/// 単位に畳んだ一覧は、確認から削除までのあいだの無関係な書き込みでfingerprintを
/// 変えない。
fn collect_ignored_paths(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
    project: &str,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
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
            "--ignored=traditional",
        ],
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            blockers.push(observation_blocker(&ignored_paths_unobservable(
                &error, project, relative,
            )));
            return;
        }
    };

    // 出力の枠組みが壊れていれば、無視対象pathが0件であることと区別できない。件数を
    // 数えずに観測不能として拒否する。
    let status = match parse_status(&outcome.stdout_text(), &IgnoredRecords::Collect) {
        Ok(status) => status,
        Err(detail) => {
            blockers.push(observation_blocker(&ignored_paths_unparseable(
                project, relative, &detail,
            )));
            return;
        }
    };
    if !status.ignored.is_empty() {
        confirmable_losses.push(ConfirmableLoss::IgnoredPaths {
            worktree: relative.to_string(),
            paths: status.ignored,
        });
    }
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
                &error,
                project,
                Fact::worktree(relative),
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
                &upstream,
                project,
                Fact::worktree(relative),
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
                &error,
                project,
                Fact::worktree(relative),
            )));
            return Observed::No;
        }
    };
    let count: u64 = if let Ok(count) = ahead.trim().parse() {
        count
    } else {
        blockers.push(observation_blocker(&local_refs_unparseable(
            project,
            Fact::worktree(relative),
            &ahead,
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

/// `commit`が、originのいずれかのremote-tracking refから到達できるか。
///
/// `git`の呼び出し先は、worktreeを見る場合と共有bare repositoryを見る場合で変わる。
/// 判定そのものは同じなので、前置の引数だけを呼び出し側から受け取る。
fn reachable_from_origin(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    git: &[&str],
    commit: &str,
    project: &str,
    subject: Fact,
    blockers: &mut Vec<Blocker>,
) -> Observed<bool> {
    let mut args = git.to_vec();
    args.extend_from_slice(&["rev-list", "--count", commit, "--not", "--remotes=origin"]);
    let unreachable = match run(host, sandbox_name, &args) {
        Ok(unreachable) => unreachable.stdout_text(),
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error, project, subject,
            )));
            return Observed::No;
        }
    };
    let count: u64 = if let Ok(count) = unreachable.trim().parse() {
        count
    } else {
        blockers.push(observation_blocker(&local_refs_unparseable(
            project,
            subject,
            &unreachable,
        )));
        return Observed::No;
    };
    Observed::Yes(count == 0)
}

/// HEAD以外のローカル所有ref（branch、tag、notes、stash）を確認対象へ分ける。
///
/// refは共有bare repositoryが持つため、worktreeごとではなくrepositoryごとに1回だけ
/// 数える。指すcommitがoriginから回収できるrefは、名前の消失を`ConfirmableLoss`
/// （確認すれば削除してよい対象）として集める。回収できないrefは`Blocker`（拒否理由）
/// として集め、確認を求めずに拒否する。どこかのworktreeがcheckout中のbranchは、
/// project metadataのstart refから再現できるため対象外とする。
fn collect_local_refs(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    repository: &RepositoryScope<'_>,
    checked_out: &BTreeSet<String>,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) {
    let outcome = match run(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            repository.git_dir,
            "for-each-ref",
            "--format=%(refname)%09%(objectname)%09%(upstream)",
            LOCAL_REF_NAMESPACES[0],
            LOCAL_REF_NAMESPACES[1],
            LOCAL_REF_NAMESPACES[2],
            LOCAL_REF_NAMESPACES[3],
        ],
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            blockers.push(observation_blocker(&reclassify_local_refs(
                &error,
                repository.project,
                Fact::root(repository.root),
            )));
            return;
        }
    };

    for line in outcome.stdout_text().lines() {
        let line = line.trim_end();
        let mut fields = line.split('\t');
        let reference = fields.next().filter(|value| !value.is_empty());
        let commit = fields.next().filter(|value| !value.is_empty());
        // 想定した形でない行を読み飛ばすと、refを1件も持たない状態と区別できない。
        let (Some(reference), Some(commit)) = (reference, commit) else {
            blockers.push(observation_blocker(&local_refs_unparseable(
                repository.project,
                Fact::root(repository.root),
                line,
            )));
            return;
        };
        let upstream = fields.next().filter(|value| !value.is_empty());

        let branch_name = reference.strip_prefix("refs/heads/");
        if branch_name.is_some_and(|branch| checked_out.contains(branch)) {
            continue;
        }

        let reachable = reachable_from_origin(
            host,
            sandbox_name,
            &["git", "--git-dir", repository.git_dir],
            commit,
            repository.project,
            Fact::root(repository.root),
            blockers,
        );
        let Observed::Yes(reachable) = reachable else {
            continue;
        };
        if !reachable {
            blockers.push(Blocker::OriginRecoveryNotProven {
                reference: reference.to_string(),
                commit: commit.to_string(),
                reason: OriginRecoveryFailure::UnreachableFromOrigin,
            });
            continue;
        }

        if let Some(name) = reference.strip_prefix("refs/tags/") {
            confirmable_losses.push(ConfirmableLoss::Tag {
                name: name.to_string(),
            });
        } else {
            confirmable_losses.push(ConfirmableLoss::LocalRef {
                reference: reference.to_string(),
            });
        }

        if let (Some(branch), Some(upstream)) = (branch_name, upstream) {
            confirmable_losses.push(ConfirmableLoss::BranchUpstream {
                branch: branch.to_string(),
                upstream: upstream.to_string(),
            });
        }
    }
}

/// originとは別の、追加のremote名を集める。remote URLは読まない。
///
/// remoteの設定は共有bare repositoryが持つため、1回だけ数える。
fn collect_additional_remotes(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    repository: &RepositoryScope<'_>,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) {
    let outcome = match run(
        host,
        sandbox_name,
        &["git", "--git-dir", repository.git_dir, "remote"],
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            blockers.push(observation_blocker(&remote_configuration_unobservable(
                &error, repository,
            )));
            return;
        }
    };

    for line in outcome.stdout_text().lines() {
        // remote名は空白を含まない。含む行は一覧が壊れている徴候であり、remoteが
        // 1件も無い状態と同じには読まない。
        let name = line.trim();
        if name.is_empty() || name.split_whitespace().count() != 1 {
            blockers.push(observation_blocker(&remote_configuration_unparseable(
                repository, line,
            )));
            return;
        }
        if name == "origin" {
            continue;
        }
        confirmable_losses.push(ConfirmableLoss::AdditionalRemote {
            name: name.to_string(),
        });
    }
}

/// どの参照からも到達できないが、reflogにだけ残るcommitを数える。
///
/// `--all`は既定で全worktreeのper-worktree refを辿るため、共有bare repositoryに対して
/// 1回だけ数えれば足りる。
fn collect_reflog_only_commits(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    repository: &RepositoryScope<'_>,
    blockers: &mut Vec<Blocker>,
    confirmable_losses: &mut Vec<ConfirmableLoss>,
) {
    let reflog = match commit_list(
        host,
        sandbox_name,
        repository,
        &["rev-list", "--walk-reflogs", "--all"],
        blockers,
    ) {
        Observed::Yes(reflog) => reflog,
        Observed::No => return,
    };
    let live = match commit_list(
        host,
        sandbox_name,
        repository,
        &["rev-list", "--all"],
        blockers,
    ) {
        Observed::Yes(live) => live,
        Observed::No => return,
    };

    let live: HashSet<&str> = live.iter().map(String::as_str).collect();
    let count = reflog
        .iter()
        .filter(|commit| !live.contains(commit.as_str()))
        .count();
    if count > 0 {
        confirmable_losses.push(ConfirmableLoss::ReflogOnlyCommits {
            count: u64::try_from(count).unwrap_or(u64::MAX),
        });
    }
}

/// `rev-list`の出力をcommit名の一覧として読む。
///
/// commit名以外の行が混ざる出力は、一覧が途中で壊れた徴候として扱い、0件と同じには
/// 読まない。
fn commit_list(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    repository: &RepositoryScope<'_>,
    rev_list: &[&str],
    blockers: &mut Vec<Blocker>,
) -> Observed<Vec<String>> {
    let mut args = vec!["git", "--git-dir", repository.git_dir];
    args.extend_from_slice(rev_list);
    let outcome = match run(host, sandbox_name, &args) {
        Ok(outcome) => outcome,
        Err(error) => {
            blockers.push(observation_blocker(&reclassify(
                &error,
                ErrorId::ReflogUnobservable,
                msg!("error-reflog-unobservable"),
                open_remediation(repository.project, msg!("remediation-reflog-unobservable")),
                Fact::root(repository.root),
            )));
            return Observed::No;
        }
    };

    let mut commits = Vec::new();
    for line in outcome.stdout_text().lines() {
        let commit = line.trim();
        if commit.is_empty() || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
            blockers.push(observation_blocker(&reflog_unparseable(repository, line)));
            return Observed::No;
        }
        commits.push(commit.to_string());
    }
    Observed::Yes(commits)
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

/// HEAD、branch、upstream、到達可能性、ローカルrefの観測が失敗した場合の共通の写像。
fn reclassify_local_refs(error: &Error, project: &str, subject: Fact) -> Error {
    reclassify(
        error,
        ErrorId::LocalRefsUnobservable,
        msg!("error-local-refs-unobservable"),
        open_remediation(project, msg!("remediation-local-refs-unobservable")),
        subject,
    )
}

/// 無視対象pathの観測が失敗した場合の写像。
fn ignored_paths_unobservable(error: &Error, project: &str, relative: &str) -> Error {
    reclassify(
        error,
        ErrorId::IgnoredPathsUnobservable,
        msg!("error-ignored-paths-unobservable"),
        open_remediation(project, msg!("remediation-ignored-paths-unobservable")),
        Fact::worktree(relative),
    )
}

/// remote構成の観測が失敗した場合の写像。
fn remote_configuration_unobservable(error: &Error, repository: &RepositoryScope<'_>) -> Error {
    reclassify(
        error,
        ErrorId::RemoteConfigurationUnobservable,
        msg!("error-remote-configuration-unobservable"),
        open_remediation(
            repository.project,
            msg!("remediation-remote-configuration-unobservable"),
        ),
        Fact::root(repository.root),
    )
}

/// commandは起動できたが、終了statusが判定対象の2値のどちらでもない場合。
fn local_refs_unobservable(outcome: &CommandOutcome, project: &str, subject: Fact) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRefsUnobservable,
            msg!("error-local-refs-unobservable"),
        )
        .fact(subject)
        .remediation(open_remediation(
            project,
            msg!("remediation-local-refs-unobservable"),
        ))
        .external(outcome.failure()),
    )
}

/// commandは成功したが、出力をrefの一覧または数値として解釈できない場合。
fn local_refs_unparseable(project: &str, subject: Fact, detail: &str) -> Error {
    unparseable(
        ErrorId::LocalRefsUnobservable,
        msg!("error-local-refs-unobservable"),
        open_remediation(project, msg!("remediation-local-refs-unobservable")),
        subject,
        detail,
    )
}

/// 無視対象pathの一覧の枠組みが壊れている場合。
fn ignored_paths_unparseable(project: &str, relative: &str, detail: &str) -> Error {
    unparseable(
        ErrorId::IgnoredPathsUnobservable,
        msg!("error-ignored-paths-unobservable"),
        open_remediation(project, msg!("remediation-ignored-paths-unobservable")),
        Fact::worktree(relative),
        detail,
    )
}

/// remote名の一覧が壊れている場合。
fn remote_configuration_unparseable(repository: &RepositoryScope<'_>, detail: &str) -> Error {
    unparseable(
        ErrorId::RemoteConfigurationUnobservable,
        msg!("error-remote-configuration-unobservable"),
        open_remediation(
            repository.project,
            msg!("remediation-remote-configuration-unobservable"),
        ),
        Fact::root(repository.root),
        detail,
    )
}

/// reflogの一覧が壊れている場合。
fn reflog_unparseable(repository: &RepositoryScope<'_>, detail: &str) -> Error {
    unparseable(
        ErrorId::ReflogUnobservable,
        msg!("error-reflog-unobservable"),
        open_remediation(repository.project, msg!("remediation-reflog-unobservable")),
        Fact::root(repository.root),
        detail,
    )
}

/// commandは成功したが、出力を想定した形として読めない場合の共通の組み立て。
fn unparseable(
    id: ErrorId,
    description: Msg,
    remediation: Remediation,
    subject: Fact,
    detail: &str,
) -> Error {
    Error::single(
        Diagnostic::new(id, description)
            .fact(subject)
            .fact(Fact::cause(&bounded(detail)))
            .remediation(remediation),
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

/// worktree一覧そのものが読めなかった場合の観測不能blocker。
fn worktree_inventory_unobservable(
    request: &Request<'_>,
    sandbox_name: &str,
    error: &Error,
) -> Blocker {
    observation_blocker(&reclassify(
        error,
        ErrorId::WorktreeInventoryUnobservable,
        msg!("error-worktree-inventory-unobservable"),
        status_remediation(
            request.metadata.display_id().as_str(),
            msg!("remediation-worktree-inventory-unobservable"),
        ),
        Fact::sandbox(sandbox_name),
    ))
}

/// worktreeを1件も観測できなかった場合の`Assessment`。`blocker`が無ければ、単に
/// この案件の作業を1つも持たないSandboxを表す。
///
/// worktreeが1件も無くても、Sandboxの書き込み層は失われる。`confirmable_losses`は
/// その1件を持ったまま渡す。
fn observation_failure(
    request: &Request<'_>,
    project: String,
    confirmable_losses: Vec<ConfirmableLoss>,
    blocker: Option<Blocker>,
) -> Assessment {
    Assessment::new(
        request.operation,
        project,
        request.sandbox.clone(),
        Vec::new(),
        blocker.into_iter().collect(),
        confirmable_losses,
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

/// hostのworkspace directoryが消えているSandboxを、共有repositoryのないSandboxと
/// 同一視せず拒否する。
///
/// 実機では、runningのままworkspace directoryだけが消えたSandboxへの`sbx exec`が
/// `422`を終了status `1`で示す。内側の`test`が「不在」を示す終了statusも`1`であり、
/// 終了statusだけではこの2つを区別できない。区別できない答えを安全側に丸めず、
/// host側を直接見て確かめられなかった場合は削除も再作成も行わない。
fn workspace_missing(request: &Request<'_>) -> Diagnostic {
    let path = sandbox::workspace_path(request.workspace_root, request.sandbox);
    let project = request.metadata.display_id();
    Diagnostic::new(
        ErrorId::SandboxWorkspaceMissing,
        msg!(
            "error-protection-workspace-missing",
            project = project.clone(),
            sandbox = request.sandbox.as_str()
        ),
    )
    .fact(Fact::path(&paths::display(&path)))
    .remediation(
        Remediation::text(msg!("remediation-sandbox-workspace-missing"))
            .try_run(format!("sbxm prepare {project}")),
    )
}
