use crate::command::HostEnvironment;
use crate::diagnostics::{ErrorId, Result};
use crate::project::SandboxLayout;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, Registered};
use crate::testing::protection::clean_host;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

fn assess(
    host: &dyn HostEnvironment,
    project: &Registered,
    operation: DestructiveOperation,
) -> Result<ProtectionAssessment> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let request = ProtectionRequest::new(operation, &project.sandbox, &layout, &project.metadata);
    gate::assess(host, request)
}

/// この観測結果自身へ明示確認し、状態が変わっていないものとして許可証を求める。
///
/// snapshot、confirmation、`gate::authorize`はどれもこのfileの外から直接組み立てられ
/// ないため、`gate::authorize`単体の挙動（blockerの有無、fingerprint一致）を確かめる
/// testはここを経由する。
fn authorize(assessment: ProtectionAssessment) -> Result<ProtectionPermit> {
    let sandbox = assessment.sandbox().as_str().to_string();
    let confirmation =
        confirmation::confirm(ProtectionSnapshot::new(assessment.clone()), &sandbox)?;
    gate::authorize(confirmation, ProtectionSnapshot::new(assessment))
}

#[test]
fn a_clean_managed_worktree_passes_and_is_reported() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = clean_host(&fixture, &project)?;

    let assessment = assess(&host, &project, DestructiveOperation::Rebuild)
        .required_because("a clean worktree passes")?;
    assert_eq!(
        assessment.worktrees(),
        [WorktreeReport {
            relative: "example-repo.tree-0".to_string(),
            kind: Kind::Managed,
            mode: Mode::Attached,
            head: COMMIT.to_string(),
            branch: Some("main".to_string()),
            remote: Remote::Pushed,
        }]
    );
    assert!(assessment.blockers().is_empty());
    authorize(assessment).required_because("no blocker means a permit is issued")?;
    Ok(())
}

#[test]
fn each_kind_of_unsaved_work_produces_its_own_blocker_and_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let tracked = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "1 .M N... 100644 100644 100644 abc abc file.txt\0",
    );
    let untracked = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? untracked.txt\0",
    );
    let unpushed = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
        0,
        "2\n",
    );
    let no_upstream = clean_host(&fixture, &project)?.answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );
    let in_progress = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        0,
        "",
    );

    let cases: [(FakeSbx, ErrorId); 5] = [
        (tracked, ErrorId::WorktreeTrackedChanges),
        (untracked, ErrorId::WorktreeUntrackedPaths),
        (unpushed, ErrorId::OriginCommitUnpushed),
        (no_upstream, ErrorId::OriginUpstreamMissing),
        (in_progress, ErrorId::GitOperationInProgress),
    ];
    for (host, expected) in cases {
        let assessment = assess(&host, &project, DestructiveOperation::Destroy)
            .required_because("assess collects the blocker instead of failing outright")?;
        let error = authorize(assessment).refused_because("unsaved work is never destroyed")?;
        assert_eq!(error.first_id(), Some(expected));
    }
    Ok(())
}

#[test]
fn untracked_paths_are_listed_in_full() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? one.txt\0? two.txt\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::UntrackedPaths {
            worktree: "example-repo.tree-0".to_string(),
            paths: vec!["one.txt".to_string(), "two.txt".to_string()],
        }]
    );
    Ok(())
}

#[test]
fn a_check_that_could_not_run_is_never_read_as_a_pass() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    // `sbx exec`が内側のcommandを起動できなかったことを示す終了status。
    let marker = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        126,
        "",
    );
    let head = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
        127,
        "",
    );
    let upstream = clean_host(&fixture, &project)?.answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            125,
            "",
        );
    let status = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        126,
        "",
    );
    let inventory = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {} worktree list --porcelain -z",
            layout.bare_git_dir()
        ),
        126,
        "",
    );

    let cases = [
        (marker, ErrorId::GitOperationUnobservable),
        (head, ErrorId::LocalRefsUnobservable),
        (upstream, ErrorId::LocalRefsUnobservable),
        (status, ErrorId::WorktreeStatusUnobservable),
        (inventory, ErrorId::WorktreeInventoryUnobservable),
    ];
    for (host, expected) in cases {
        let error = assess(&host, &project, DestructiveOperation::Destroy)
            .refused_because("a check that did not answer never means the worktree is safe")?;
        assert_eq!(error.first_id(), Some(expected));
    }
    Ok(())
}

#[test]
fn an_unmanaged_worktree_is_refused_for_rebuild_and_confirmable_for_destroy() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let extra = format!("{}/agent-scratch", layout.bare_root());

    let host = clean_host(&fixture, &project)?
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
                &format!("{COMMIT}\n"),
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

    let assessment = assess(&host, &project, DestructiveOperation::Rebuild)
        .required_because("assess still succeeds; the blocker is collected")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::UnmanagedWorktree {
            worktree: "agent-scratch".to_string()
        }]
    );
    let error = authorize(assessment)
        .refused_because("rebuild cannot recreate a worktree it does not know about")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnmanagedWorktreePresent));

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("destroy examines it under the same content rules")?;
    assert!(assessment.blockers().is_empty());
    assert_eq!(assessment.worktrees().len(), 2);
    assert_eq!(assessment.worktrees()[1].kind, Kind::Unmanaged);
    assert_eq!(assessment.worktrees()[1].remote, Remote::Reachable);
    authorize(assessment).required_because("no blocker means destroy may still proceed")?;
    Ok(())
}

#[test]
fn a_worktree_that_is_not_an_artifact_of_this_project_is_not_reported_as_unsaved_work() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    // bare rootの外を指すworktree。
    let outside = clean_host(&fixture, &project)?.answering(
        &listing,
        0,
        &format!(
            "worktree {}\0bare\0\0worktree /home/agent/elsewhere\0branch refs/heads/main\0\0",
            layout.bare_root()
        ),
    );
    let error = assess(&outside, &project, DestructiveOperation::Destroy)
        .refused_because("a path outside the repository is a security refusal")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeOutsideRepository));
    Ok(())
}

#[test]
fn a_sandbox_whose_git_lists_no_worktree_has_nothing_to_lose() -> Checked {
    // 構築や再構築が途中で終わったSandboxには、checkoutされた作業が存在しない。
    // 宣言との食い違いを理由に止めると、作り直す手段がなくなる。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    let empty = clean_host(&fixture, &project)?.answering(
        &listing,
        0,
        &format!("worktree {}\0bare\0\0", layout.bare_root()),
    );
    let assessment = assess(&empty, &project, DestructiveOperation::Rebuild)
        .required_because("a sandbox holding no worktree can be replaced")?;
    assert!(assessment.worktrees().is_empty());
    Ok(())
}

#[test]
fn a_detached_head_that_no_remote_reaches_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::OriginRecoveryNotProven {
            reference: "HEAD".to_string(),
            commit: COMMIT.to_string(),
            reason: OriginRecoveryFailure::UnreachableFromOrigin,
        }]
    );
    let error =
        authorize(assessment).refused_because("commits no remote holds are not thrown away")?;
    assert_eq!(error.first_id(), Some(ErrorId::OriginCommitUnreachable));
    Ok(())
}

#[test]
fn a_sandbox_without_the_shared_repository_has_nothing_to_lose() -> Checked {
    // 構築が途中で終わったSandboxには、この案件の作業が1件もない。worktreeが
    // 観測できないことを、失うものがある徴候として読まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {}", layout.bare_git_dir()),
        1,
        "",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Rebuild)
        .required_because("a sandbox that holds no repository can be replaced")?;
    assert!(assessment.worktrees().is_empty());
    Ok(())
}

#[test]
fn a_git_directory_that_cannot_be_read_stops_before_the_markers_are_checked() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-parse --git-dir"),
        1,
        "",
    );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("the git directory could not be resolved")?;
    assert_eq!(error.first_id(), Some(ErrorId::GitOperationUnobservable));
    Ok(())
}

#[test]
fn a_head_commit_that_cannot_be_read_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-parse HEAD"),
        1,
        "",
    );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("HEAD could not be resolved")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn an_ahead_count_that_cannot_be_read_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
        1,
        "",
    );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("the ahead count could not be read")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn an_ahead_count_that_is_not_a_number_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
        0,
        "not-a-number\n",
    );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("an unparseable count is never read as zero")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn an_unreachable_count_that_cannot_be_read_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            1,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-list --count HEAD --not --remotes=origin"
            ),
            1,
            "",
        );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("the unreachable count could not be read")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn an_unreachable_count_that_is_not_a_number_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
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
            "not-a-number\n",
        );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("an unparseable count is never read as zero")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn a_rename_entry_counts_as_a_tracked_change_and_consumes_its_original_path_field() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "2 R. N... 100644 100644 100644 abc abc R100 new.txt\0old.txt\0! ignored.txt\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::TrackedChanges {
            worktree: "example-repo.tree-0".to_string(),
        }]
    );
    Ok(())
}

#[test]
fn a_sandbox_whose_existence_cannot_be_observed_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = InnerCommandSandbox::new().timing_out(&format!("test -e {}", layout.bare_git_dir()));

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("existence that could not be observed is never read as absent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}

#[test]
fn whether_head_is_attached_cannot_be_observed_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = InnerCommandSandbox::new()
        .holding(&[bare_git_dir.as_str()])
        .answering(
            &format!("git --git-dir {bare_git_dir} worktree list --porcelain -z"),
            &format!(
                "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!("git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
            "",
        )
        .answering(
            &format!("git -C {managed} rev-parse --git-dir"),
            &format!("{managed}/.git"),
        )
        .answering(&format!("git -C {managed} rev-parse HEAD"), COMMIT)
        .timing_out(&format!(
            "git -C {managed} symbolic-ref --quiet --short HEAD"
        ));

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("whether HEAD is attached could not be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn whether_an_upstream_is_configured_cannot_be_observed_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = InnerCommandSandbox::new()
        .holding(&[bare_git_dir.as_str()])
        .answering(
            &format!("git --git-dir {bare_git_dir} worktree list --porcelain -z"),
            &format!(
                "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!("git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
            "",
        )
        .answering(
            &format!("git -C {managed} rev-parse --git-dir"),
            &format!("{managed}/.git"),
        )
        .answering(&format!("git -C {managed} rev-parse HEAD"), COMMIT)
        .answering(
            &format!("git -C {managed} symbolic-ref --quiet --short HEAD"),
            "main",
        )
        .timing_out(&format!(
            "git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
        ));

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("whether an upstream is configured could not be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}
