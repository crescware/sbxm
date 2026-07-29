//! 共通のデータ保護検査。
//!
//! runningなSandboxを削除する通常modeの`rebuild`と`destroy`は、同じ列挙と判定規則で
//! 保存されていない作業がないことを確かめる。判定できない場合は削除しない。

use std::collections::BTreeSet;

use crate::command::{CommandOutcome, HostEnvironment};
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxLayout;

use super::sandbox;
use super::worktree;

/// 進行中のGit操作を示すfile。1つでもあれば削除しない。
const IN_PROGRESS_MARKERS: [&str; 6] = [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "BISECT_LOG",
    "rebase-merge",
    "rebase-apply",
];

/// unmanaged worktreeの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmanaged {
    /// `destroy`。保存状態を満たせば削除して良い。
    Allowed,
    /// `rebuild`。配置を再現できないため、存在するだけで拒否する。
    Refused,
}

/// metadataとの対応。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Managed,
    Unmanaged,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Managed => "managed",
            Kind::Unmanaged => "unmanaged",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Kind::Managed => "legend-managed",
            Kind::Unmanaged => "legend-unmanaged",
        }
    }
}

/// HEADの持ち方。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Attached,
    Detached,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Attached => "attached",
            Mode::Detached => "detached",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Mode::Attached => "legend-attached",
            Mode::Detached => "legend-detached",
        }
    }
}

/// commitがremoteへ渡っている根拠。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remote {
    Pushed,
    Reachable,
}

impl Remote {
    pub fn as_str(self) -> &'static str {
        match self {
            Remote::Pushed => "pushed",
            Remote::Reachable => "reachable",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Remote::Pushed => "legend-pushed",
            Remote::Reachable => "legend-reachable",
        }
    }
}

/// worktree 1件の観測結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeReport {
    /// bare rootからの相対path。
    pub relative: String,
    pub kind: Kind,
    pub mode: Mode,
    pub head: String,
    /// attached modeのbranch名。
    pub branch: Option<String>,
    pub remote: Remote,
}

impl WorktreeReport {
    /// この行が使った状態値と、その説明のmessage ID。
    pub fn legends(&self) -> [(&'static str, &'static str); 3] {
        [
            (self.kind.as_str(), self.kind.legend_id()),
            (self.mode.as_str(), self.mode.legend_id()),
            (self.remote.as_str(), self.remote.legend_id()),
        ]
    }
}

/// 検査結果。
#[derive(Debug, Clone)]
pub struct Protection {
    pub worktrees: Vec<WorktreeReport>,
}

/// active session、worktree、保存状態を検査する。
///
/// 1件でも条件を満たさない場合は、対象を示して拒否する。
pub fn inspect(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    unmanaged: Unmanaged,
) -> Result<Protection> {
    let bare_root = layout.bare_root();
    // 共有repositoryのないSandboxは、この案件の作業を1つも持たない。worktreeが観測
    // できないことを、失うものがある徴候として読まない。構築が途中で終わったSandboxが
    // これにあたる。
    if !sandbox::path_exists(host, sandbox_name, &layout.bare_git_dir())? {
        return Ok(Protection {
            worktrees: Vec::new(),
        });
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

    Ok(Protection { worktrees })
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
    if !status
        .stdout_text()
        .trim_matches(['\0', '\n', ' '])
        .is_empty()
    {
        return Err(refuse(msg!(
            "error-unsaved-work-uncommitted",
            target = relative
        )));
    }

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
        if ahead != "0" {
            return Err(refuse(msg!(
                "error-unsaved-work-unpushed",
                target = relative,
                count = ahead,
                upstream = upstream
            )));
        }
        (Mode::Attached, Some(branch), Remote::Pushed)
    } else {
        // detached HEADは、originのいずれかのrefから到達できることを条件とする。
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
        if unreachable != "0" {
            return Err(refuse(msg!(
                "error-unsaved-work-unreachable",
                target = relative
            )));
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

/// Sandbox内の検査commandが答えた終了status。
///
/// `sbx exec`がcommandを起動できなかった場合を、内側のcommandが返した結果として
/// 読まない。判定できない場合は、削除して良いことを示す値へ丸めずerrorとする。
fn answered(outcome: &CommandOutcome, subject: &str) -> Result<i32> {
    sandbox::inner_exit_code(outcome).ok_or_else(|| sandbox::unobservable(outcome, subject))
}

/// 保存されていない作業を失わないため、削除も再作成も行わない。
///
/// 拒否理由は利用者向けの本文であり、選択した言語で読めるmessageとして渡す。
fn refuse(reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::UnsavedWork, reason).remediation(msg!("remediation-unsaved-work")),
    )
}

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;
