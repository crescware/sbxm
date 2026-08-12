use crate::command::HostEnvironment;
use crate::design::Document;
use crate::design::policy::StreamPolicy;
use crate::design::renderer::Renderer;
use crate::diagnostics::{ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::project::{SandboxLayout, SandboxName};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::protection::clean_host;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

fn assess(
    host: &dyn HostEnvironment,
    project: &Registered,
    operation: DestructiveOperation,
) -> Result<Assessment> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let request = Request::new(operation, &project.sandbox, &layout, &project.metadata);
    gate::assess(host, &request)
}

fn render_diagnostic(
    diagnostic: &crate::diagnostics::Diagnostic,
    locale: Locale,
) -> Checked<String> {
    let mut bytes = Vec::new();
    let document = Document::new().diagnostic(diagnostic.clone());
    {
        let mut renderer = Renderer::new(&mut bytes, StreamPolicy::plain());
        renderer.write(&Catalog::new(locale), &document);
    }
    String::from_utf8(bytes).required_because("the diagnostic renderer writes UTF-8")
}

fn blocker_diagnostic(blocker: Blocker) -> Checked<crate::diagnostics::Diagnostic> {
    let sandbox = SandboxName::derive(&project_id("example-org/example-repo")?.canonical());
    let assessment = Assessment::new(
        DestructiveOperation::Destroy,
        "example-org/example-repo".to_string(),
        sandbox,
        Vec::new(),
        vec![blocker],
        Vec::new(),
    );
    let error =
        gate::require_no_blockers(&assessment).refused_because("the blocker is rendered")?;
    error
        .diagnostics()
        .first()
        .cloned()
        .required_because("one blocker produces one diagnostic")
}

fn assert_protection_diagnostic(
    blocker: Blocker,
    id: ErrorId,
    command: &str,
    labels: &[&str],
) -> Checked {
    let diagnostic = blocker_diagnostic(blocker)?;
    assert_eq!(diagnostic.id, id);
    assert!(
        diagnostic.description.args.is_empty(),
        "diagnostic values belong in named facts: {diagnostic:?}"
    );
    let fact_labels: Vec<&str> = diagnostic
        .facts
        .iter()
        .map(|fact| fact.label().id)
        .collect();
    for label in labels {
        assert!(
            fact_labels.contains(label),
            "missing {label}: {diagnostic:?}"
        );
    }
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("every blocker has remediation")?;
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(crate::design::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec![command]
    );
    for locale in [Locale::En, Locale::Ja] {
        let drawn = render_diagnostic(&diagnostic, locale)?;
        assert!(drawn.contains(command), "{locale:?}: {drawn:?}");
        assert!(!drawn.contains("destroy --force"), "{drawn:?}");
        assert!(!drawn.contains("git clean"), "{drawn:?}");
        assert!(!drawn.contains("git reset --hard"), "{drawn:?}");
    }
    Ok(())
}

#[test]
fn protection_diagnostics_render_named_facts_and_safe_commands_in_both_locales() -> Checked {
    assert_protection_diagnostic(
        Blocker::TrackedChanges {
            worktree: "example-repo.tree-0".to_string(),
        },
        ErrorId::WorktreeTrackedChanges,
        "sbxm open example-org/example-repo",
        &["diagnostic-worktree-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::UntrackedPaths {
            worktree: "example-repo.tree-0".to_string(),
            paths: vec!["one.txt".to_string(), "two.txt".to_string()],
        },
        ErrorId::WorktreeUntrackedPaths,
        "sbxm open example-org/example-repo",
        &["diagnostic-worktree-label", "diagnostic-paths-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::GitOperationInProgress {
            worktree: "example-repo.tree-0".to_string(),
            operation: "MERGE_HEAD".to_string(),
        },
        ErrorId::GitOperationInProgress,
        "sbxm open example-org/example-repo",
        &["diagnostic-worktree-label", "diagnostic-operation-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::UnmanagedWorktree {
            worktree: "agent-scratch".to_string(),
        },
        ErrorId::UnmanagedWorktreePresent,
        "sbxm status example-org/example-repo",
        &["diagnostic-worktree-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::WorktreeOutsideRepository {
            path: "/home/agent/elsewhere".to_string(),
            root: "/home/agent/work/example-org/example-repo".to_string(),
        },
        ErrorId::WorktreeOutsideRepository,
        "sbxm status example-org/example-repo",
        &["diagnostic-path-label", "diagnostic-root-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::OriginRecoveryNotProven {
            reference: "main".to_string(),
            commit: COMMIT.to_string(),
            reason: OriginRecoveryFailure::NoUpstream,
        },
        ErrorId::OriginUpstreamMissing,
        "sbxm open example-org/example-repo",
        &["diagnostic-reference-label", "diagnostic-commit-label"],
    )?;
    assert_protection_diagnostic(
        Blocker::OriginRecoveryNotProven {
            reference: "main".to_string(),
            commit: COMMIT.to_string(),
            reason: OriginRecoveryFailure::AheadOfUpstream {
                upstream: "origin/main".to_string(),
                count: 2,
            },
        },
        ErrorId::OriginCommitUnpushed,
        "sbxm open example-org/example-repo",
        &[
            "diagnostic-reference-label",
            "diagnostic-commit-label",
            "diagnostic-upstream-label",
            "diagnostic-count-label",
        ],
    )?;
    assert_protection_diagnostic(
        Blocker::OriginRecoveryNotProven {
            reference: "HEAD".to_string(),
            commit: COMMIT.to_string(),
            reason: OriginRecoveryFailure::UnreachableFromOrigin,
        },
        ErrorId::OriginCommitUnreachable,
        "sbxm open example-org/example-repo",
        &["diagnostic-reference-label", "diagnostic-commit-label"],
    )?;
    Ok(())
}

/// この観測結果自身へ明示確認し、状態が変わっていないものとして許可証を求める。
///
/// snapshot、confirmation、`gate::authorize`はどれもこのfileの外から直接組み立てられ
/// ないため、`gate::authorize`単体の挙動（blockerの有無、fingerprint一致）を確かめる
/// testはここを経由する。
fn authorize(assessment: Assessment) -> Result<ProtectionPermit> {
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
        [Blocker::UntrackedPaths {
            worktree: "example-repo.tree-0".to_string(),
            paths: vec!["one.txt".to_string(), "two.txt".to_string()],
        }]
    );
    Ok(())
}

#[test]
fn an_unknown_status_record_is_never_read_as_clean() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "x unexpected-record\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an unknown status record is retained as an observation blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("an unknown status record is not evidence of a clean worktree")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeStatusUnobservable));
    let diagnostic = error
        .diagnostics()
        .first()
        .cloned()
        .required_because("the refusal carries one diagnostic")?;
    let drawn = render_diagnostic(&diagnostic, Locale::En)?;
    assert!(
        drawn.contains("x unexpected-record"),
        "the offending record is shown as a cause: {drawn:?}"
    );
    Ok(())
}

#[test]
fn an_incomplete_status_record_is_never_read_as_clean() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "2 R. N... 100644 100644 100644 abc abc R100 new.txt\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an incomplete status record is retained as an observation blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("a rename without its original path is not a valid status record")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeStatusUnobservable));
    let diagnostic = error
        .diagnostics()
        .first()
        .cloned()
        .required_because("the refusal carries one diagnostic")?;
    let drawn = render_diagnostic(&diagnostic, Locale::En)?;
    assert!(
        drawn.contains("R100 new.txt"),
        "the offending record is shown as a cause: {drawn:?}"
    );
    Ok(())
}

#[test]
fn an_ignored_path_record_is_never_read_as_clean() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    // `--ignored`を渡していないため、実際のgitはこの種別のrecordを出さない。それでも
    // 既知の無害なrecordとして素通りさせないことを固定する。
    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "! ignored.txt\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an ignored-path record is retained as an observation blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("an ignored-path record is not evidence of a clean worktree")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeStatusUnobservable));
    Ok(())
}

#[test]
fn untracked_paths_beyond_the_display_cap_are_summarized_by_count() -> Checked {
    let paths: Vec<String> = (0..25)
        .map(|index| format!("generated-{index}.txt"))
        .collect();
    let diagnostic = blocker_diagnostic(Blocker::UntrackedPaths {
        worktree: "example-repo.tree-0".to_string(),
        paths: paths.clone(),
    })?;
    let drawn = render_diagnostic(&diagnostic, Locale::En)?;
    assert!(drawn.contains(&paths[0]), "{drawn:?}");
    assert!(drawn.contains(&paths[19]), "{drawn:?}");
    assert!(!drawn.contains(&paths[20]), "{drawn:?}");
    assert!(drawn.contains("25"), "the total count is shown: {drawn:?}");
    Ok(())
}

#[test]
fn multiple_blockers_are_collected_in_stable_observation_order() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "1 .M N... 100644 100644 100644 abc abc tracked.txt\0? loose.txt\0",
        )
        .answering(&format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"), 0, "")
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("all known blockers are collected")?;
    assert_eq!(
        assessment.blockers(),
        [
            Blocker::TrackedChanges {
                worktree: "example-repo.tree-0".to_string(),
            },
            Blocker::UntrackedPaths {
                worktree: "example-repo.tree-0".to_string(),
                paths: vec!["loose.txt".to_string()],
            },
            Blocker::GitOperationInProgress {
                worktree: "example-repo.tree-0".to_string(),
                operation: "MERGE_HEAD".to_string(),
            },
            Blocker::OriginRecoveryNotProven {
                reference: "main".to_string(),
                commit: COMMIT.to_string(),
                reason: OriginRecoveryFailure::NoUpstream,
            },
        ]
    );

    let error = gate::require_no_blockers(&assessment)
        .refused_because("multiple blockers are reported together")?;
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.id)
            .collect::<Vec<_>>(),
        vec![
            ErrorId::WorktreeTrackedChanges,
            ErrorId::WorktreeUntrackedPaths,
            ErrorId::GitOperationInProgress,
            ErrorId::OriginUpstreamMissing,
        ]
    );
    Ok(())
}

#[test]
fn an_observation_failure_does_not_hide_later_blockers() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    // statusで既知の変更を収集した後、最初のmarker検査だけが起動不能になる。
    // それでも残りのmarkerとorigin検査を続け、結果を観測順にすべて保持する。
    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "1 .M N... 100644 100644 100644 abc abc tracked.txt\0",
        )
        .answering(&format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"), 126, "")
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("all independent checks contribute to the assessment")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("observed and unobservable blockers are reported together")?;
    let ids = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.id)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 3);
    assert_eq!(
        ids,
        vec![
            ErrorId::WorktreeTrackedChanges,
            ErrorId::GitOperationUnobservable,
            ErrorId::OriginUpstreamMissing,
        ]
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
        let assessment = assess(&host, &project, DestructiveOperation::Destroy)
            .required_because("an unanswered check is retained as an observation blocker")?;
        let error = gate::require_no_blockers(&assessment)
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
        [Blocker::UnmanagedWorktree {
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
    let assessment = assess(&outside, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [Blocker::WorktreeOutsideRepository {
            path: "/home/agent/elsewhere".to_string(),
            root: layout.bare_root(),
        }]
    );
    let error = gate::require_no_blockers(&assessment)
        .refused_because("a path outside the repository is a security refusal")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeOutsideRepository));
    Ok(())
}

#[test]
fn a_worktree_outside_the_repository_is_collected_alongside_other_blockers() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                layout.bare_git_dir()
            ),
            0,
            &format!(
                "worktree {}\0bare\0\0worktree /home/agent/elsewhere\0branch refs/heads/main\0\0worktree {managed}\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "1 .M N... 100644 100644 100644 abc abc file.txt\0",
        );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("a boundary refusal does not stop other worktrees from being examined")?;
    assert_eq!(
        assessment.blockers(),
        [
            Blocker::WorktreeOutsideRepository {
                path: "/home/agent/elsewhere".to_string(),
                root: layout.bare_root(),
            },
            Blocker::TrackedChanges {
                worktree: "example-repo.tree-0".to_string(),
            },
        ]
    );
    let error = gate::require_no_blockers(&assessment)
        .refused_because("both blockers are reported together")?;
    assert_eq!(
        error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.id)
            .collect::<Vec<_>>(),
        vec![
            ErrorId::WorktreeOutsideRepository,
            ErrorId::WorktreeTrackedChanges,
        ]
    );
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
        [Blocker::OriginRecoveryNotProven {
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("the unreadable git directory is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("the unreadable HEAD is retained as a blocker")?;
    let error =
        gate::require_no_blockers(&assessment).refused_because("HEAD could not be resolved")?;
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("the unreadable ahead count is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an unparseable ahead count is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("the unreadable unreachable count is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an unparseable unreachable count is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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
        "2 R. N... 100644 100644 100644 abc abc R100 new.txt\0old.txt\0",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker")?;
    assert_eq!(
        assessment.blockers(),
        [Blocker::TrackedChanges {
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("unobserved existence is retained as an inventory blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("existence that could not be observed is never read as absent")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::WorktreeInventoryUnobservable)
    );
    Ok(())
}

#[test]
fn an_existence_probe_that_cannot_start_is_not_read_as_absent() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {}", layout.bare_git_dir()),
        126,
        "",
    );

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("an unstartable existence probe is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("an unstartable existence probe is not absence")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::WorktreeInventoryUnobservable)
    );
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("unobserved HEAD attachment is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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

    let assessment = assess(&host, &project, DestructiveOperation::Destroy)
        .required_because("unobserved upstream state is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("whether an upstream is configured could not be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}
