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
    let declared: BTreeSet<&str> = metadata
        .managed_worktrees
        .iter()
        .map(|worktree| worktree.path.as_str())
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
        let managed = declared.contains(relative.as_str());
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

    // Gitがworktreeを1つも持たないSandboxには、checkoutされた作業が存在しない。
    // 宣言との食い違いはstatusが示す問題であり、ここで止めても守るものがない。構築や
    // 再構築が途中で終わったSandboxがこれにあたり、止めると作り直す手段がなくなる。
    if worktrees.is_empty() {
        return Ok(Protection { worktrees });
    }

    let diagnose = msg!(
        "remediation-managed-worktree-missing",
        command = format!("sbxm status {}", metadata.display_id())
    );
    for declared in &metadata.managed_worktrees {
        if !seen.contains(declared.path.as_str()) {
            // metadataとGitの食い違いであり、保存されていない作業とは別の事象である。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::ManagedWorktreeMissing,
                    msg!("error-managed-worktree-missing", path = declared.path),
                )
                .remediation(diagnose),
            ));
        }
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

    let git_dir = read(
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
            _ => return Err(unobservable(&probe, &candidate)),
        }
    }

    let head = read(
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
        _ => return Err(unobservable(&branch, "HEAD")),
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
        let ahead = read(
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
        let unreachable = read(
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
    sandbox::inner_exit_code(outcome).ok_or_else(|| unobservable(outcome, subject))
}

/// 内側のcommandが答えなかった場合の診断。原値をそのまま残す。
fn unobservable(outcome: &CommandOutcome, subject: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxCheckUnobservable,
            msg!(
                "error-sandbox-check-unobservable",
                subject = subject,
                exit_status = outcome.status
            ),
        )
        .external(outcome.failure()),
    )
}

fn read(host: &dyn HostEnvironment, sandbox_name: &str, args: &[&str]) -> Result<String> {
    let outcome = sandbox::exec(host, sandbox_name, args)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
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
pub mod tests {
    use super::*;
    use crate::workflow::inventory::tests::{FakeSbx, Fixture, Registered, fixture};

    /// 検査を通るworktreeを持つhost。
    pub fn clean_host(fixture: &Fixture, project: &Registered) -> FakeSbx {
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());
        FakeSbx::listing(&format!("[{}]", fixture.entry(project, "running")))
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                    layout.bare_git_dir()
                ),
                0,
                &format!(
                    "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0",
                    layout.bare_root()
                ),
            )
            .answering(
                &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
                0,
                "",
            )
            .answering(
                &format!("exec {name} -- git -C {managed} rev-parse --git-dir"),
                0,
                &format!("{managed}/.git\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {managed} rev-parse HEAD"),
                0,
                "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5\n",
            )
            .answering(
                &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
                0,
                "main\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
                ),
                0,
                "origin/main\n",
            )
            .answering(
                &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
                0,
                "0\n",
            )
            // 進行中のGit操作を示すfileはない。
            .answering(&format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {managed}/.git/CHERRY_PICK_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {managed}/.git/REVERT_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {managed}/.git/BISECT_LOG"), 1, "")
            .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-merge"), 1, "")
            .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-apply"), 1, "")
    }

    fn inspect_with(
        host: &FakeSbx,
        project: &Registered,
        unmanaged: Unmanaged,
    ) -> Result<Protection> {
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        inspect(
            host,
            project.sandbox.as_str(),
            &layout,
            &project.metadata,
            unmanaged,
        )
    }

    #[test]
    fn a_clean_managed_worktree_passes_and_is_reported() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = clean_host(&fixture, &project);

        let protection =
            inspect_with(&host, &project, Unmanaged::Refused).expect("a clean worktree passes");
        assert_eq!(
            protection.worktrees,
            vec![WorktreeReport {
                relative: "example-repo.tree-0".to_string(),
                kind: Kind::Managed,
                mode: Mode::Attached,
                head: "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5".to_string(),
                branch: Some("main".to_string()),
                remote: Remote::Pushed,
            }]
        );
    }

    #[test]
    fn work_that_is_not_committed_or_not_pushed_stops_the_run() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());

        let dirty = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "? untracked.txt\0",
        );
        let unpushed = clean_host(&fixture, &project).answering(
            &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
            0,
            "2\n",
        );
        let no_upstream = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );
        let in_progress = clean_host(&fixture, &project).answering(
            &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
            0,
            "",
        );

        for host in [dirty, unpushed, no_upstream, in_progress] {
            let error = inspect_with(&host, &project, Unmanaged::Refused)
                .expect_err("unsaved work is never destroyed");
            assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
        }
    }

    #[test]
    fn a_check_that_could_not_run_is_never_read_as_a_pass() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());

        // `sbx exec`が内側のcommandを起動できなかったことを示す終了status。
        let marker = clean_host(&fixture, &project).answering(
            &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
            126,
            "",
        );
        let head = clean_host(&fixture, &project).answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            127,
            "",
        );
        let upstream = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            125,
            "",
        );

        for host in [marker, head, upstream] {
            let error = inspect_with(&host, &project, Unmanaged::Allowed)
                .expect_err("a check that did not answer never means the worktree is safe");
            assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
        }
    }

    #[test]
    fn an_unmanaged_worktree_is_refused_for_rebuild_and_examined_for_destroy() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());
        let extra = format!("{}/agent-scratch", layout.bare_root());

        let host = clean_host(&fixture, &project)
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                    layout.bare_git_dir()
                ),
                0,
                &format!(
                    "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0worktree {extra}\0detached\0\0",
                    layout.bare_root()
                ),
            )
            .answering(
                &format!("exec {name} -- git -C {extra} status --porcelain=v2 -z --untracked-files=all"),
                0,
                "",
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-parse --git-dir"),
                0,
                &format!("{extra}/.git\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-parse HEAD"),
                0,
                "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5\n",
            )
            .answering(
                &format!("exec {name} -- git -C {extra} symbolic-ref --quiet --short HEAD"),
                1,
                "",
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-list --count HEAD --not --remotes=origin"),
                0,
                "0\n",
            )
            .answering(&format!("exec {name} -- test -e {extra}/.git/MERGE_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/CHERRY_PICK_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/REVERT_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/BISECT_LOG"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-merge"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-apply"), 1, "");

        let error = inspect_with(&host, &project, Unmanaged::Refused)
            .expect_err("rebuild cannot recreate a worktree it does not know about");
        assert_eq!(error.first_id(), Some(ErrorId::UnmanagedWorktreePresent));

        let protection = inspect_with(&host, &project, Unmanaged::Allowed)
            .expect("destroy examines it under the same rules");
        assert_eq!(protection.worktrees.len(), 2);
        assert_eq!(protection.worktrees[1].kind, Kind::Unmanaged);
        assert_eq!(protection.worktrees[1].remote, Remote::Reachable);
    }

    #[test]
    fn a_worktree_that_is_not_an_artifact_of_this_project_is_not_reported_as_unsaved_work() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let listing = format!(
            "exec {name} -- git --git-dir {} worktree list --porcelain -z",
            layout.bare_git_dir()
        );

        // bare rootの外を指すworktree。
        let outside = clean_host(&fixture, &project).answering(
            &listing,
            0,
            &format!(
                "worktree {}\0bare\0\0worktree /home/agent/elsewhere\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        );
        let error = inspect_with(&outside, &project, Unmanaged::Allowed)
            .expect_err("a path outside the repository is a security refusal");
        assert_eq!(error.first_id(), Some(ErrorId::WorktreeOutsideRepository));

        // 宣言したworktreeのうち1つだけをGitが持っていない。作業のあるSandboxで
        // metadataとGitが食い違っている状態であり、削除の前に解消させる。
        let mut two_declared = project.clone();
        two_declared
            .metadata
            .managed_worktrees
            .push(crate::metadata::ManagedWorktree {
                path: "example-repo.tree-1".to_string(),
                created_from: "refs/remotes/origin/main".to_string(),
            });
        let error = inspect_with(
            &clean_host(&fixture, &project),
            &two_declared,
            Unmanaged::Allowed,
        )
        .expect_err("the declaration and Git disagree");
        assert_eq!(error.first_id(), Some(ErrorId::ManagedWorktreeMissing));
    }

    #[test]
    fn a_sandbox_whose_git_lists_no_worktree_has_nothing_to_lose() {
        // 構築や再構築が途中で終わったSandboxには、checkoutされた作業が存在しない。
        // 宣言との食い違いを理由に止めると、作り直す手段がなくなる。
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let listing = format!(
            "exec {name} -- git --git-dir {} worktree list --porcelain -z",
            layout.bare_git_dir()
        );

        let empty = clean_host(&fixture, &project).answering(
            &listing,
            0,
            &format!("worktree {}\0bare\0\0", layout.bare_root()),
        );
        let protection = inspect_with(&empty, &project, Unmanaged::Refused)
            .expect("a sandbox holding no worktree can be replaced");
        assert!(protection.worktrees.is_empty());
    }

    #[test]
    fn a_detached_head_that_no_remote_reaches_stops_the_run() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());

        let host = clean_host(&fixture, &project)
            .answering(
                &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
                1,
                "",
            )
            .answering(
                &format!(
                    "exec {name} -- git -C {managed} rev-list --count HEAD --not --remotes=origin"
                ),
                0,
                "3\n",
            );

        let error = inspect_with(&host, &project, Unmanaged::Allowed)
            .expect_err("commits no remote holds are not thrown away");
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    }

    #[test]
    fn a_sandbox_without_the_shared_repository_has_nothing_to_lose() {
        // 構築が途中で終わったSandboxには、この案件の作業が1件もない。worktreeが
        // 観測できないことを、失うものがある徴候として読まない。
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let name = project.sandbox.as_str();
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let host = clean_host(&fixture, &project).answering(
            &format!("exec {name} -- test -e {}", layout.bare_git_dir()),
            1,
            "",
        );

        let protection = inspect_with(&host, &project, Unmanaged::Refused)
            .expect("a sandbox that holds no repository can be replaced");
        assert!(protection.worktrees.is_empty());
    }
}
