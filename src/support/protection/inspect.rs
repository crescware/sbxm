use std::collections::BTreeSet;
use std::path::Path;

use crate::command::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::sandbox;
use crate::support::worktree;

use super::{BARE_GIT_DIR_PROBE, Kind, Mode, Remote, Report, Unmanaged, WorktreeReport, answered};

/// 進行中のGit操作を示すfile。1つでもあれば削除しない。
const IN_PROGRESS_MARKERS: [&str; 6] = [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "BISECT_LOG",
    "rebase-merge",
    "rebase-apply",
];

/// active session、worktree、保存状態を検査する。
///
/// 1件でも条件を満たさない場合は、対象を示して拒否する。
pub fn inspect(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    workspace_root: &Path,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    unmanaged: Unmanaged,
) -> Result<Report> {
    let sandbox_name = sandbox.as_str();
    let bare_root = layout.bare_root();

    // mount元が無いSandboxへの`sbx exec`は、内側のcommandを起動できないまま終了status
    // だけを返す。その終了statusは、内側のcommandが答えた「不在」と区別できない
    // (詳細は`workspace_missing`)。sbx execの答えへ頼る前に、mount元をhostで直接見る。
    if !sandbox::workspace_exists(workspace_root, sandbox)? {
        return Err(workspace_missing(metadata, sandbox, workspace_root));
    }

    // 共有repositoryのないSandboxは、この案件の作業を1つも持たない。worktreeが観測
    // できないことを、失うものがある徴候として読まない。構築が途中で終わったSandboxが
    // これにあたる。
    //
    // ただし、直前のhost側確認とこの`sbx exec`の間にもworkspace directoryが消えうる。
    // その場合`sbx exec`は内側のshellを起動できないまま終了statusだけを返し、その値は
    // `test -e`が答える`0`/`1`と重なるため、終了statusだけでは区別できない
    // (詳細は`BARE_GIT_DIR_PROBE`)。内側のshellが実際に走った場合だけstdoutへ書かれる
    // 印が無ければ、終了statusを`test`の答えとして読まない。
    let bare_git_dir = layout.bare_git_dir();
    let probe = sandbox::exec(
        host,
        sandbox_name,
        &["sh", "-c", BARE_GIT_DIR_PROBE, "sh", &bare_git_dir],
    )?;
    if probe.stdout_text().is_empty() {
        return Err(sandbox::unobservable(&probe, &bare_git_dir));
    }
    match answered(&probe, &bare_git_dir)? {
        0 => {}
        1 => {
            return Ok(Report {
                worktrees: Vec::new(),
            });
        }
        _ => return Err(sandbox::unobservable(&probe, &bare_git_dir)),
    }
    let entries = worktree::list(host, sandbox_name, layout)?;
    let declared: BTreeSet<String> = layout
        .worktree_names(metadata.provisioning.requested_worktrees)
        .into_iter()
        .collect();
    let mut worktrees = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for entry in entries {
        if entry.bare {
            continue;
        }
        let Some(relative) = entry.relative_to(&bare_root) else {
            // bare root外のworktreeは、案件の成果物として扱えない。保存状態の問題ではない。
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
        if !managed && unmanaged == Unmanaged::Refused {
            // 保存状態にかかわらず拒否する。commitしても解消しないため案内も分ける。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::UnmanagedWorktreePresent,
                    msg!("error-unmanaged-worktree-present", path = relative),
                )
                .remediation(msg!("remediation-unmanaged-worktree-present")),
            ));
        }

        seen.insert(relative.clone());
        worktrees.push(examine(host, sandbox_name, &entry, &relative, managed)?);
    }

    Ok(Report { worktrees })
}

/// 1件のworktreeが、保存されていない作業を持たないことを確かめる。
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &worktree::Entry,
    relative: &str,
    managed: bool,
) -> Result<WorktreeReport> {
    let path = entry.path.as_str();

    require_clean_tree(host, sandbox_name, path, relative)?;
    require_no_operation_in_progress(host, sandbox_name, path, relative)?;

    let head = sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "HEAD"],
    )?;
    let branch = sandbox::exec(
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
    )?;

    // `symbolic-ref --quiet`はdetached HEADを`1`で示す。それ以外の終了statusは判定しない。
    let attached = match answered(&branch, "HEAD")? {
        0 => true,
        1 => false,
        _ => return Err(sandbox::unobservable(&branch, "HEAD")),
    };

    let (mode, branch, remote) = if attached {
        let branch = branch.stdout_text().trim().to_string();
        require_pushed(host, sandbox_name, path, relative)?;
        (Mode::Attached, Some(branch), Remote::Pushed)
    } else {
        require_reachable_from_origin(host, sandbox_name, path, relative)?;
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

/// 未commitの変更も未追跡fileも無いこと。
fn require_clean_tree(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<()> {
    let status = sandbox::exec(
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
    )?
    .require_success()?;
    if status
        .stdout_text()
        .trim_matches(['\0', '\n', ' '])
        .is_empty()
    {
        return Ok(());
    }
    Err(refuse(msg!(
        "error-unsaved-work-uncommitted",
        target = relative
    )))
}

/// merge、rebase、cherry-pickのような操作が途中で止まっていないこと。
fn require_no_operation_in_progress(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<()> {
    let git_dir = sandbox::read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "--git-dir"],
    )?;
    for marker in IN_PROGRESS_MARKERS {
        let candidate = format!("{git_dir}/{marker}");
        let probe = sandbox::exec(host, sandbox_name, &["test", "-e", &candidate])?;
        // `test`はfileの不在を`1`で示す。commandを起動できなかったことを不在として読まない。
        match answered(&probe, &candidate)? {
            0 => {
                return Err(refuse(msg!(
                    "error-unsaved-work-in-progress",
                    target = relative,
                    operation = marker
                )));
            }
            1 => {}
            _ => return Err(sandbox::unobservable(&probe, &candidate)),
        }
    }
    Ok(())
}

/// upstreamがあり、そこへ載っていないcommitを持たないこと。
fn require_pushed(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<()> {
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
    )?;
    // upstream未設定はgitが非ゼロで示す。起動できなかった場合と区別する。
    if answered(&upstream, "@{upstream}")? != 0 {
        return Err(refuse(msg!(
            "error-unsaved-work-no-upstream",
            target = relative
        )));
    }
    let upstream = upstream.stdout_text().trim().to_string();
    let ahead = sandbox::read(
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
    )?;
    if ahead == "0" {
        return Ok(());
    }
    Err(refuse(msg!(
        "error-unsaved-work-unpushed",
        target = relative,
        count = ahead,
        upstream = upstream
    )))
}

/// detached HEADが、originのいずれかのrefから到達できること。
fn require_reachable_from_origin(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    path: &str,
    relative: &str,
) -> Result<()> {
    let unreachable = sandbox::read(
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
    )?;
    if unreachable == "0" {
        return Ok(());
    }
    Err(refuse(msg!(
        "error-unsaved-work-unreachable",
        target = relative
    )))
}

/// 保存されていない作業を失わないため、削除も再作成も行わない。
///
/// 拒否理由は利用者向けの本文であり、選択した言語で読めるmessageとして渡す。
fn refuse(reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::UnsavedWork, reason).remediation(msg!("remediation-unsaved-work")),
    )
}

/// hostのworkspace directoryが消えているSandboxを、共有repositoryのないSandboxと
/// 同一視せず拒否する。
///
/// 実機では、runningのままworkspace directoryだけが消えたSandboxへの`sbx exec`が
/// `422`を終了status `1`で示す。内側の`test`が「不在」を示す終了statusも`1`であり、
/// 終了statusだけではこの2つを区別できない。区別できない答えを安全側に丸めず、
/// host側を直接見て確かめられなかった場合は削除も再作成も行わない。
fn workspace_missing(
    metadata: &ProjectMetadata,
    sandbox: &SandboxName,
    workspace_root: &Path,
) -> Error {
    let path = sandbox::workspace_path(workspace_root, sandbox);
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxWorkspaceMissing,
            msg!(
                "error-protection-workspace-missing",
                project = metadata.display_id(),
                sandbox = sandbox.as_str()
            ),
        )
        .fact(Fact::path(&paths::display(&path)))
        .remediation(
            Remediation::text(msg!("remediation-sandbox-workspace-missing"))
                .try_run(format!("sbxm prepare {}", metadata.display_id())),
        ),
    )
}
