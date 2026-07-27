//! 共通のデータ保護検査。
//!
//! runningなSandboxを削除する通常modeの`rebuild`と`destroy`は、同じ列挙と判定規則で
//! 保存されていない作業がないことを確かめる。判定できない場合は削除しない。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths;
use crate::project::SandboxLayout;

use super::daemon;
use super::sandbox;

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

/// worktree 1件の観測結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeReport {
    /// bare rootからの相対path。
    pub relative: String,
    /// 翻訳しない`managed`または`unmanaged`。
    pub kind: &'static str,
    /// 翻訳しない`attached`または`detached`。
    pub mode: &'static str,
    pub head: String,
    /// attached modeのbranch名。
    pub branch: Option<String>,
    /// 翻訳しない`clean`または`dirty`。
    pub state: &'static str,
    /// remoteへ渡っているか。翻訳しない`pushed`、`reachable`、`unpushed`。
    pub remote: &'static str,
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
    require_no_active_session(host, sandbox_name)?;

    let git_dir = layout.bare_git_dir();
    let bare_root = layout.bare_root();
    let listing = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ],
    )?
    .require_success()?;

    let entries = super::status_project::parse_worktree_list(&listing.stdout_text())?;
    let mut worktrees = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for entry in entries {
        if entry.bare {
            continue;
        }
        let standardized = paths::lexically_standardize(std::path::Path::new(&entry.path));
        let Ok(relative) = standardized.strip_prefix(&bare_root) else {
            // bare root外のworktreeは、案件の成果物として扱えない。
            return Err(refuse(
                &entry.path,
                "the worktree is outside the shared repository",
            ));
        };
        let relative = paths::display(relative);
        let managed = metadata
            .managed_worktrees
            .iter()
            .any(|worktree| worktree.path == relative);
        if !managed && unmanaged == Unmanaged::Refused {
            return Err(refuse(
                &relative,
                "this worktree is not part of the target configuration, and its layout cannot be recreated",
            ));
        }

        seen.push(relative.clone());
        worktrees.push(examine(host, sandbox_name, &entry, &relative, managed)?);
    }

    for declared in &metadata.managed_worktrees {
        if !seen.contains(&declared.path) {
            return Err(refuse(
                &declared.path,
                "the metadata declares this managed worktree, but Git does not have it",
            ));
        }
    }
    if worktrees.is_empty() {
        return Err(refuse(
            &bare_root,
            "no worktree was found, so the saved state cannot be judged",
        ));
    }

    Ok(Protection { worktrees })
}

/// 1件のworktreeが、保存されていない作業を持たないことを確かめる。
fn examine(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    entry: &super::status_project::WorktreeEntry,
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
        return Err(refuse(
            relative,
            "the working tree has changes that are not committed",
        ));
    }

    let git_dir = read(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "--git-dir"],
    )?;
    for marker in IN_PROGRESS_MARKERS {
        let candidate = format!("{git_dir}/{marker}");
        if sandbox::exec(host, sandbox_name, &["test", "-e", &candidate])?.success() {
            return Err(refuse(
                relative,
                &format!("a Git operation is in progress ({marker})"),
            ));
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

    let (mode, branch, remote) = if branch.success() {
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
        if !upstream.success() {
            return Err(refuse(
                relative,
                "the branch has no upstream, so its commits exist only in the sandbox",
            ));
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
            return Err(refuse(
                relative,
                &format!("{ahead} commits are not pushed to {upstream}"),
            ));
        }
        ("attached", Some(branch), "pushed")
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
            return Err(refuse(
                relative,
                "the detached HEAD holds commits that no remote branch reaches",
            ));
        }
        ("detached", None, "reachable")
    };

    Ok(WorktreeReport {
        relative: relative.to_string(),
        kind: if managed { "managed" } else { "unmanaged" },
        mode,
        head,
        branch,
        state: "clean",
        remote,
    })
}

/// 対象Sandboxにsessionが接続していないこと。
fn require_no_active_session(host: &dyn HostEnvironment, sandbox_name: &str) -> Result<()> {
    let entries = daemon::list(host)?;
    let Some(entry) = entries.iter().find(|entry| entry.name == sandbox_name) else {
        return Ok(());
    };
    match entry.active_sessions {
        Some(0) => Ok(()),
        Some(count) => Err(Error::single(
            Diagnostic::new(
                ErrorId::DaemonSessionActive,
                msg!(
                    "error-daemon-session-active",
                    sandboxes = format!("{sandbox_name} ({count})")
                ),
            )
            .remediation(msg!("remediation-daemon-session-active")),
        )),
        None => Err(Error::single(
            Diagnostic::new(
                ErrorId::DaemonSessionUnobservable,
                msg!("error-daemon-session-unobservable", sandbox = sandbox_name),
            )
            .remediation(msg!("remediation-daemon-session-unobservable")),
        )),
    }
}

fn read(host: &dyn HostEnvironment, sandbox_name: &str, args: &[&str]) -> Result<String> {
    let outcome = sandbox::exec(host, sandbox_name, args)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}

/// 保存されていない作業を失わないため、削除も再作成も行わない。
fn refuse(target: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::UnsavedWork,
            msg!("error-unsaved-work", target = target, detail = detail),
        )
        .remediation(msg!("remediation-unsaved-work")),
    )
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::workflow::inventory::tests::{FakeSbx, Fixture, fixture};

    /// 検査を通るworktreeを持つhost。
    pub fn clean_host(
        fixture: &Fixture,
        project: &crate::workflow::inventory::ManagedProject,
    ) -> FakeSbx {
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
        project: &crate::workflow::inventory::ManagedProject,
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
                kind: "managed",
                mode: "attached",
                head: "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5".to_string(),
                branch: Some("main".to_string()),
                state: "clean",
                remote: "pushed",
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
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));

        let protection = inspect_with(&host, &project, Unmanaged::Allowed)
            .expect("destroy examines it under the same rules");
        assert_eq!(protection.worktrees.len(), 2);
        assert_eq!(protection.worktrees[1].kind, "unmanaged");
        assert_eq!(protection.worktrees[1].remote, "reachable");
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
    fn a_connected_session_or_a_version_that_hides_them_stops_the_run() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let name = project.sandbox.as_str();
        let workspace = fixture.workspace_root.join(name);
        std::fs::create_dir_all(&workspace).unwrap();
        let template = crate::workflow::image::image_name(
            &project.sandbox,
            &project.metadata.provisioning.dockerfile_sha256,
        );

        let busy = FakeSbx::listing(&format!(
            r#"[{{"name":"{name}","state":"running","workspace":"{}","template":"{template}","active_sessions":1}}]"#,
            workspace.display()
        ));
        let error = inspect_with(&busy, &project, Unmanaged::Allowed)
            .expect_err("a connected session is never dropped");
        assert_eq!(error.first_id(), Some(ErrorId::DaemonSessionActive));

        let hidden = FakeSbx::listing(&format!(
            r#"[{{"name":"{name}","state":"running","workspace":"{}","template":"{template}"}}]"#,
            workspace.display()
        ));
        let error = inspect_with(&hidden, &project, Unmanaged::Allowed)
            .expect_err("absence of sessions has to be observed");
        assert_eq!(error.first_id(), Some(ErrorId::DaemonSessionUnobservable));
    }
}
