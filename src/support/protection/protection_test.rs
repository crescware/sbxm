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
            reachability: Reachability::Pushed {
                upstream: "refs/remotes/origin/main".to_string(),
            },
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
    let in_progress = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        0,
        "",
    );

    let cases: [(FakeSbx, ErrorId); 3] = [
        (tracked, ErrorId::WorktreeTrackedChanges),
        (untracked, ErrorId::WorktreeUntrackedPaths),
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
fn a_branch_without_an_upstream_that_another_origin_ref_reaches_passes() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/release\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/release\n",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy).required_because(
        "a commit reachable from any origin ref is safe, even without an upstream",
    )?;
    assert!(assessment.blockers().is_empty());
    assert_eq!(
        assessment.worktrees()[0].reachability,
        Reachability::Reachable {
            origins: vec!["refs/remotes/origin/release".to_string()]
        }
    );
    Ok(())
}

#[test]
fn a_branch_whose_upstream_does_not_reach_it_but_another_origin_ref_does_passes() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();

    // upstreamは`refs/remotes/origin/main`だが、そのtipは別commitで、HEADへは
    // `refs/remotes/origin/release`からしか到達できない。
    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/main\tdef456\nrefs/remotes/origin/release\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/release\n",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("a commit reachable from a non-upstream origin ref is still safe")?;
    assert!(assessment.blockers().is_empty());
    assert_eq!(
        assessment.worktrees()[0].reachability,
        Reachability::Reachable {
            origins: vec!["refs/remotes/origin/release".to_string()]
        }
    );
    Ok(())
}

#[test]
fn a_branch_whose_commit_no_origin_ref_reaches_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();

    let host = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
        ),
        0,
        "",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::OriginUnreachable {
            reference: "refs/heads/main".to_string(),
            commit: COMMIT.to_string(),
        }]
    );
    let error = authorize(assessment).refused_because(
        "upstream not proven ahead does not matter once origin truly cannot reach it",
    )?;
    assert_eq!(error.first_id(), Some(ErrorId::OriginCommitUnreachable));
    Ok(())
}

#[test]
fn origin_unobservable_reasons_each_produce_their_own_blocker() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();

    let origin_missing = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} config --get remote.origin.url"),
        1,
        "",
    );
    let refresh_failed = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} fetch --prune origin"),
        128,
        "",
    );
    let advertisement_invalid = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
        ),
        0,
        "not-tab-separated\n",
    );
    let object_missing = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
        ),
        128,
        "",
    );

    let cases = [
        (
            origin_missing,
            UnobservableReason::OriginMissing,
            ErrorId::OriginMissing,
        ),
        (
            refresh_failed,
            UnobservableReason::RefreshFailed,
            ErrorId::OriginRefreshFailed,
        ),
        (
            advertisement_invalid,
            UnobservableReason::AdvertisementInvalid,
            ErrorId::OriginAdvertisementInvalid,
        ),
        (
            object_missing,
            UnobservableReason::ObjectMissing,
            ErrorId::OriginObjectMissing,
        ),
    ];
    for (host, reason, expected_id) in cases {
        let assessment = assess(&host, &project, DestructiveOperation::Destroy).required_because(
            "an unobservable origin is a collected blocker, not an outright failure",
        )?;
        assert_eq!(
            assessment.blockers(),
            [ProtectionBlocker::OriginUnobservable {
                reference: "refs/heads/main".to_string(),
                commit: COMMIT.to_string(),
                reason,
            }]
        );
        let error = authorize(assessment)
            .refused_because("an unobservable origin never permits a normal destroy/rebuild")?;
        assert_eq!(error.first_id(), Some(expected_id));
    }
    Ok(())
}

#[test]
fn local_refs_other_than_the_current_branch_are_classified_independently() -> Checked {
    const TAG_COMMIT: &str = "1111111111111111111111111111111111111111";
    const FEATURE_COMMIT: &str = "2222222222222222222222222222222222222222";
    const ORPHAN_COMMIT: &str = "3333333333333333333333333333333333333333";

    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git -C {managed} for-each-ref --format=%(refname)%09%(objectname)%09%(upstream) refs/heads/ refs/tags/ refs/notes/ refs/stash"
            ),
            0,
            &format!(
                "\nrefs/heads/nocommit\t\t\nrefs/heads/main\t{COMMIT}\t\nrefs/tags/v1\t{TAG_COMMIT}\t\nrefs/heads/feature\t{FEATURE_COMMIT}\trefs/remotes/origin/feature\nrefs/heads/orphan\t{ORPHAN_COMMIT}\t\n"
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!(
                "refs/remotes/origin/main\t{COMMIT}\nrefs/remotes/origin/feature\t{FEATURE_COMMIT}\n"
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={TAG_COMMIT} refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/main\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={FEATURE_COMMIT} refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/feature\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={ORPHAN_COMMIT} refs/remotes/origin/"
            ),
            0,
            "",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("origin-reachable local refs are confirmable, not a reason to refuse")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::OriginUnreachable {
            reference: "refs/heads/orphan".to_string(),
            commit: ORPHAN_COMMIT.to_string(),
        }]
    );
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::Tag {
                worktree: "example-repo.tree-0".to_string(),
                name: "v1".to_string(),
            })
    );
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::LocalRef {
                worktree: "example-repo.tree-0".to_string(),
                reference: "refs/heads/feature".to_string(),
            })
    );
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::BranchUpstream {
                worktree: "example-repo.tree-0".to_string(),
                branch: "feature".to_string(),
                upstream: "refs/remotes/origin/feature".to_string(),
            })
    );
    Ok(())
}

#[test]
fn an_additional_remote_is_confirmable_and_its_absence_is_never_assumed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} remote"),
        0,
        "origin\nupstream\n",
    );
    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an additional remote name is only confirmable")?;
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::AdditionalRemote {
                worktree: "example-repo.tree-0".to_string(),
                name: "upstream".to_string(),
            })
    );

    let unobservable = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} remote"),
        1,
        "",
    );
    let error = assess(&unobservable, &project, DestructiveOperation::Destroy)
        .refused_because("remote configuration that could not be read is never assumed empty")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::RemoteConfigurationUnobservable)
    );
    Ok(())
}

#[test]
fn an_ignored_path_that_is_present_is_a_confirmable_loss() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all --ignored=matching"
        ),
        0,
        "! .env\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an ignored path does not block, it is only confirmed")?;
    assert!(assessment.blockers().is_empty());
    assert_eq!(
        assessment.confirmable_losses(),
        [
            ConfirmableLoss::IgnoredPaths {
                worktree: "example-repo.tree-0".to_string(),
                paths: vec![".env".to_string()],
            },
            ConfirmableLoss::SandboxWritableLayer,
        ]
    );

    let unobservable = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all --ignored=matching"
        ),
        126,
        "",
    );
    let error = assess(&unobservable, &project, DestructiveOperation::Destroy)
        .refused_because("ignored paths that could not be read are never assumed absent")?;
    assert_eq!(error.first_id(), Some(ErrorId::IgnoredPathsUnobservable));
    Ok(())
}

#[test]
fn reflog_only_commits_are_counted_and_their_absence_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!("exec {name} -- git -C {managed} rev-list --walk-reflogs --all"),
            0,
            &format!("{COMMIT}\nabandoned0000000000000000000000000000000\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-list --all"),
            0,
            &format!("{COMMIT}\n"),
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("a commit only reflogs hold is confirmable, not a block")?;
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::ReflogOnlyCommits {
                worktree: "example-repo.tree-0".to_string(),
                count: 1,
            })
    );

    let unobservable = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-list --walk-reflogs --all"),
        126,
        "",
    );
    let error = assess(&unobservable, &project, DestructiveOperation::Destroy)
        .refused_because("a reflog that could not be read is never assumed empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::ReflogUnobservable));
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
        &format!("exec {name} -- git -C {managed} rev-parse --symbolic-full-name @{{upstream}}"),
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
    assert_eq!(
        assessment.worktrees()[1].reachability,
        Reachability::Reachable {
            origins: vec!["refs/remotes/origin/main".to_string()]
        }
    );
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
    let bare_git_dir = layout.bare_git_dir();

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            1,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            0,
            "",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [ProtectionBlocker::OriginUnreachable {
            reference: "HEAD".to_string(),
            commit: COMMIT.to_string(),
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
fn whether_an_origin_ref_reaches_a_commit_that_cannot_be_read_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();

    let host = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
        ),
        126,
        "",
    );

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("a command that could not even launch is never read as observed")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
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
            "git -C {managed} rev-parse --symbolic-full-name @{{upstream}}"
        ));

    let error = assess(&host, &project, DestructiveOperation::Destroy)
        .refused_because("whether an upstream is configured could not be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}
