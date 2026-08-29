use crate::boundary::host::HostEnvironment;
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

fn snapshot(
    host: &dyn HostEnvironment,
    fixture: &Fixture,
    project: &Registered,
    operation: DestructiveOperation,
) -> Result<ProtectionSnapshot> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let request = Request::new(
        operation,
        &project.sandbox,
        &fixture.workspace_root,
        &layout,
        &project.metadata,
    );
    gate::assess(host, &request)
}

/// 観測結果だけを見るtestのための取り出し。
///
/// `gate::assess`が返すのは`ProtectionSnapshot`であり、`Assessment`を直接返す入口は
/// 無い。collectorの出力だけを確かめるtestは、そこから観測結果を借りて読む。
fn assess(
    host: &dyn HostEnvironment,
    fixture: &Fixture,
    project: &Registered,
    operation: DestructiveOperation,
) -> Result<Assessment> {
    Ok(snapshot(host, fixture, project, operation)?
        .assessment()
        .clone())
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
        None,
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
    assert_protection_diagnostic_with_commands(blocker, id, &[command], labels)
}

fn assert_protection_diagnostic_with_commands(
    blocker: Blocker,
    id: ErrorId,
    commands: &[&str],
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
        commands
    );
    for locale in [Locale::En, Locale::Ja] {
        let drawn = render_diagnostic(&diagnostic, locale)?;
        for command in commands {
            assert!(drawn.contains(command), "{locale:?}: {drawn:?}");
        }
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
        Blocker::OriginUnreachable {
            reference: "HEAD".to_string(),
            commit: COMMIT.to_string(),
        },
        ErrorId::OriginCommitUnreachable,
        "sbxm open example-org/example-repo",
        &["diagnostic-reference-label", "diagnostic-commit-label"],
    )?;
    for (reason, id) in [
        (UnobservableReason::OriginMissing, ErrorId::OriginMissing),
        (
            UnobservableReason::AdvertisementInvalid,
            ErrorId::OriginAdvertisementInvalid,
        ),
        (
            UnobservableReason::ObjectMissing,
            ErrorId::OriginObjectMissing,
        ),
    ] {
        assert_protection_diagnostic(
            Blocker::OriginUnobservable {
                references: vec!["refs/heads/main".to_string()],
                reason,
            },
            id,
            "sbxm open example-org/example-repo",
            &["diagnostic-references-label"],
        )?;
    }
    // originのrefreshそのものが失敗した場合だけ、この案件のstatusとhost全体のstatusの
    // 両方を示す。
    assert_protection_diagnostic_with_commands(
        Blocker::OriginUnobservable {
            references: vec!["refs/heads/main".to_string()],
            reason: UnobservableReason::RefreshFailed,
        },
        ErrorId::OriginRefreshFailed,
        &[
            "sbxm status --global",
            "sbxm status example-org/example-repo",
        ],
        &["diagnostic-references-label"],
    )?;
    Ok(())
}

#[test]
fn an_unobservable_blocker_beyond_the_listing_cap_adds_a_count_fact() -> Checked {
    let references: Vec<String> = (0..25)
        .map(|index| format!("refs/heads/f{index}"))
        .collect();
    let diagnostic = blocker_diagnostic(Blocker::OriginUnobservable {
        references,
        reason: UnobservableReason::OriginMissing,
    })?;
    let fact_labels: Vec<&str> = diagnostic
        .facts
        .iter()
        .map(|fact| fact.label().id)
        .collect();
    assert!(fact_labels.contains(&"diagnostic-references-label"));
    assert!(fact_labels.contains(&"diagnostic-count-label"));
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
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
        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!("refs/sbxm/origin/heads/release\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/release\n",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because(
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
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!("refs/sbxm/origin/heads/main\tdef456\nrefs/sbxm/origin/heads/release\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/release\n",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
        ),
        0,
        "",
    );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [Blocker::OriginUnreachable {
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
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
        ),
        128,
        "",
    );
    let advertisement_invalid = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
        ),
        0,
        "not-tab-separated\n",
    );
    let object_missing = clean_host(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            128,
            "",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {bare_git_dir} cat-file -e {COMMIT}"),
            1,
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
        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
            .required_because(
                "an unobservable origin is a collected blocker, not an outright failure",
            )?;
        assert_eq!(
            assessment.blockers(),
            [Blocker::OriginUnobservable {
                references: vec!["refs/heads/main".to_string()],
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
    // checkout中のbranchも含め、tag・notes・stash・未checkoutのbranch・custom namespaceを
    // 同じ観測結果で判定する。1本でも回収できないrefがあれば、確認を求めず拒否する。
    const TAG_COMMIT: &str = "1111111111111111111111111111111111111111";
    const FEATURE_COMMIT: &str = "2222222222222222222222222222222222222222";
    const ORPHAN_COMMIT: &str = "3333333333333333333333333333333333333333";
    const CUSTOM_COMMIT: &str = "4444444444444444444444444444444444444444";

    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();

    let host = clean_host(&fixture, &project)?
        .answering(
            &repository_command(
                &project,
                "for-each-ref --format=%(refname)%09%(objectname)%09%(upstream) refs/",
            ),
            0,
            &format!(
                "refs/heads/main\t{COMMIT}\trefs/remotes/origin/main\nrefs/tags/v1\t{TAG_COMMIT}\t\nrefs/heads/feature\t{FEATURE_COMMIT}\trefs/remotes/origin/feature\nrefs/heads/orphan\t{ORPHAN_COMMIT}\t\nrefs/custom/local\t{CUSTOM_COMMIT}\t\n"
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!(
                "refs/sbxm/origin/heads/main	{COMMIT}\nrefs/sbxm/origin/heads/feature	{FEATURE_COMMIT}\n"
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={TAG_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/main\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={FEATURE_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/feature\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={ORPHAN_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={CUSTOM_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/custom/remote\n",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("origin-reachable local refs are confirmable, not a reason to refuse")?;
    assert_eq!(
        assessment.blockers(),
        [Blocker::OriginUnreachable {
            reference: "refs/heads/orphan".to_string(),
            commit: ORPHAN_COMMIT.to_string(),
        }]
    );
    // checkout中のbranchも、削除時にはlocal ref名とupstream追跡を失うため数える。
    assert_eq!(
        assessment.confirmable_losses(),
        [
            ConfirmableLoss::SandboxWritableLayer,
            ConfirmableLoss::LocalRef {
                reference: "refs/heads/main".to_string(),
            },
            ConfirmableLoss::BranchUpstream {
                branch: "main".to_string(),
                upstream: "refs/remotes/origin/main".to_string(),
            },
            ConfirmableLoss::Tag {
                name: "v1".to_string(),
            },
            ConfirmableLoss::LocalRef {
                reference: "refs/heads/feature".to_string(),
            },
            ConfirmableLoss::BranchUpstream {
                branch: "feature".to_string(),
                upstream: "refs/remotes/origin/feature".to_string(),
            },
            ConfirmableLoss::LocalRef {
                reference: "refs/custom/local".to_string(),
            },
        ]
    );
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
                "exec {name} -- git --git-dir {} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/",
                SandboxLayout::new(project.metadata.canonical_id()).bare_git_dir()
            ),
            0,
            "",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
            Blocker::OriginUnreachable {
                reference: "refs/heads/main".to_string(),
                commit: COMMIT.to_string(),
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
            ErrorId::OriginCommitUnreachable,
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
                "exec {name} -- git --git-dir {} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/",
                SandboxLayout::new(project.metadata.canonical_id()).bare_git_dir()
            ),
            0,
            "",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
            ErrorId::OriginCommitUnreachable,
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
        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
            .answering(&format!("exec {name} -- test -e {extra}/.git/MERGE_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/CHERRY_PICK_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/REVERT_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/BISECT_LOG"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-merge"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-apply"), 1, "");

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
    // destroyは存在自体を削除計画へ載せ、確認の対象にする。
    assert!(
        assessment
            .confirmable_losses()
            .contains(&ConfirmableLoss::UnmanagedWorktree {
                worktree: "agent-scratch".to_string()
            }),
        "{:?}",
        assessment.confirmable_losses()
    );
    authorize(assessment).required_because("no blocker means destroy may still proceed")?;
    Ok(())
}

/// 共有bare repositoryへ問い合わせるcommandのkey。
fn repository_command(project: &Registered, rest: &str) -> String {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    format!(
        "exec {} -- git --git-dir {} {rest}",
        project.sandbox.as_str(),
        layout.bare_git_dir()
    )
}

/// 層Bの確認対象を一通り持つhost。
///
/// 無視対象path、checkout中のbranchとそのupstream、未checkoutのbranch、tag、notes、stash、
/// 追加remote、reflogにだけ残るcommitを揃える。
fn host_with_every_layer_b_loss(fixture: &Fixture, project: &Registered) -> Checked<FakeSbx> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let other = "0123456789abcdef0123456789abcdef01234567";
    let dangling = "fedcba9876543210fedcba9876543210fedcba98";

    Ok(clean_host(fixture, project)?
        .answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --ignored=traditional"
            ),
            0,
            "! node_modules/\0! target/\0",
        )
        .answering(
            &repository_command(
                project,
                "for-each-ref --format=%(refname)%09%(objectname)%09%(upstream) refs/",
            ),
            0,
            &format!(
                "refs/heads/main\t{COMMIT}\torigin/main\n\
                 refs/heads/topic\t{COMMIT}\torigin/topic\n\
                 refs/tags/v1\t{COMMIT}\t\n\
                 refs/notes/commits\t{COMMIT}\t\n\
                 refs/stash\t{COMMIT}\t\n"
            ),
        )
        .answering(
            &repository_command(
                project,
                &format!("rev-list --count {COMMIT} --not --remotes=origin"),
            ),
            0,
            "0\n",
        )
        .answering(&repository_command(project, "remote"), 0, "origin\nfork\n")
        .answering(
            &repository_command(project, "rev-list --walk-reflogs --all"),
            0,
            &format!("{COMMIT}\n{other}\n{dangling}\n"),
        )
        .answering(
            &repository_command(project, "rev-list --all"),
            0,
            &format!("{COMMIT}\n"),
        ))
}

#[test]
fn every_kind_of_layer_b_loss_reaches_the_deletion_plan() -> Checked {
    // #82: 層Aを通過したあとにも失われるものは、削除計画へ1件残らず現れる。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = host_with_every_layer_b_loss(&fixture, &project)?;

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("every layer B collector answers")?;
    assert!(assessment.blockers().is_empty());
    assert_eq!(
        assessment.confirmable_losses(),
        [
            // Sandboxの書き込み層は、観測の成否によらず必ず失われる。
            ConfirmableLoss::SandboxWritableLayer,
            ConfirmableLoss::IgnoredPaths {
                worktree: "example-repo.tree-0".to_string(),
                paths: vec!["node_modules/".to_string(), "target/".to_string()],
            },
            // branchはcheckout中かどうかにかかわらず、名前とupstream追跡を分けて数える。
            ConfirmableLoss::LocalRef {
                reference: "refs/heads/main".to_string(),
            },
            ConfirmableLoss::BranchUpstream {
                branch: "main".to_string(),
                upstream: "origin/main".to_string(),
            },
            ConfirmableLoss::LocalRef {
                reference: "refs/heads/topic".to_string(),
            },
            ConfirmableLoss::BranchUpstream {
                branch: "topic".to_string(),
                upstream: "origin/topic".to_string(),
            },
            ConfirmableLoss::Tag {
                name: "v1".to_string(),
            },
            // notesとstashは名前で特別扱いせず、ローカル所有refとして同じ形で数える。
            ConfirmableLoss::LocalRef {
                reference: "refs/notes/commits".to_string(),
            },
            ConfirmableLoss::LocalRef {
                reference: "refs/stash".to_string(),
            },
            ConfirmableLoss::AdditionalRemote {
                name: "fork".to_string(),
            },
            ConfirmableLoss::ReflogOnlyCommits { count: 2 },
        ]
    );
    Ok(())
}

#[test]
fn a_repository_wide_loss_is_counted_once_however_many_worktrees_there_are() -> Checked {
    // ref、tag、remote、reflogは共有bare repositoryが持つ。worktreeごとに数えると、
    // 同じtagを何度も見せ、他のworktreeがcheckoutしているbranchまで損失に数えてしまう。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let second = format!("{}/example-repo.tree-1", layout.bare_root());

    let host = host_with_every_layer_b_loss(&fixture, &project)?
        .answering(
            &format!(
                "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                layout.bare_git_dir()
            ),
            0,
            &format!(
                "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0worktree {second}\0branch refs/heads/topic\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!("exec {name} -- git -C {second} status --porcelain=v2 -z --untracked-files=all"),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git -C {second} status --porcelain=v2 -z --ignored=traditional"),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git -C {second} rev-parse --git-dir"),
            0,
            &format!("{second}/.git\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {second} rev-parse HEAD"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {second} symbolic-ref --quiet --short HEAD"),
            0,
            "topic\n",
        )
        .answering(
            &format!(
                "exec {name} -- git -C {second} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            0,
            "origin/topic\n",
        )
        .answering(
            &format!("exec {name} -- git -C {second} rev-list --count origin/topic..HEAD"),
            0,
            "0\n",
        )
        .answering(&format!("exec {name} -- test -e {second}/.git/MERGE_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {second}/.git/CHERRY_PICK_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {second}/.git/REVERT_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {second}/.git/BISECT_LOG"), 1, "")
        .answering(&format!("exec {name} -- test -e {second}/.git/rebase-merge"), 1, "")
        .answering(&format!("exec {name} -- test -e {second}/.git/rebase-apply"), 1, "");

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("both worktrees are examined")?;
    assert_eq!(assessment.worktrees().len(), 2);

    let losses = assessment.confirmable_losses();
    assert_eq!(
        losses
            .iter()
            .filter(|loss| matches!(loss, ConfirmableLoss::Tag { .. }))
            .count(),
        1,
        "a tag belongs to the repository, not to a worktree: {losses:?}"
    );
    assert_eq!(
        losses
            .iter()
            .filter(|loss| matches!(loss, ConfirmableLoss::AdditionalRemote { .. }))
            .count(),
        1,
        "a remote belongs to the repository as well: {losses:?}"
    );
    assert_eq!(
        losses
            .iter()
            .filter(|loss| matches!(loss, ConfirmableLoss::ReflogOnlyCommits { .. }))
            .count(),
        1,
        "the reflog is walked once for the whole repository: {losses:?}"
    );
    // topicは2つ目のworktreeがcheckoutしていても、local ref名として損失に数える。
    assert!(
        losses.iter().any(|loss| matches!(
            loss,
            ConfirmableLoss::LocalRef { reference } if reference == "refs/heads/topic"
        )),
        "a checked-out branch is still a lost local name: {losses:?}"
    );
    Ok(())
}

#[test]
fn a_sandbox_without_the_shared_repository_still_loses_its_writable_layer() -> Checked {
    // 構築が途中で終わったSandboxにも、Gitの外へ書かれたものは残る。repositoryを
    // 観測できないことを、失うものが何も無いことと同じには読まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {}", layout.bare_git_dir()),
        1,
        "",
    );

    let present = snapshot(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("a sandbox that holds no repository can still be destroyed")?;
    assert_eq!(
        present.assessment().confirmable_losses(),
        [ConfirmableLoss::SandboxWritableLayer]
    );

    // 「Sandboxがそもそも無い」観測とは別の状態である。同じfingerprintになると、確認から
    // 削除までのあいだにSandboxが現れても素通りしてしまう。
    let absent = gate::assess_absent(
        DestructiveOperation::Destroy,
        project.metadata.display_id(),
        &project.sandbox,
    );
    assert!(absent.assessment().confirmable_losses().is_empty());
    assert_ne!(
        present.fingerprint(),
        absent.fingerprint(),
        "a sandbox that exists is never the same state as one that does not"
    );

    let confirmation = confirmation::confirm(absent, project.sandbox.as_str())
        .required_because("the plan said there was nothing to lose")?;
    let error = gate::authorize(confirmation, present)
        .refused_because("the sandbox appeared after the plan was confirmed")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionStateChanged));
    Ok(())
}

#[test]
fn a_layer_b_collector_that_cannot_answer_names_the_inventory_it_could_not_read() -> Checked {
    // #82: 層B inventoryを一部でも観測できなければ、削除計画が完全であると証明できない。
    // 汎用のIDへ丸めず、どの一覧を読み直せばよいかをerror codeで示す。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let ignored =
        format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --ignored=traditional");
    let refs = repository_command(
        &project,
        "for-each-ref --format=%(refname)%09%(objectname)%09%(upstream) refs/",
    );
    let remotes = repository_command(&project, "remote");
    let reflog = repository_command(&project, "rev-list --walk-reflogs --all");

    let cases = [
        (
            clean_host(&fixture, &project)?.answering(&ignored, 128, ""),
            ErrorId::IgnoredPathsUnobservable,
        ),
        // 枠組みの壊れた出力は、無視対象pathが0件であることと区別できない。
        (
            clean_host(&fixture, &project)?.answering(&ignored, 0, "! node_modules/"),
            ErrorId::IgnoredPathsUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&refs, 128, ""),
            ErrorId::LocalRefsUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&refs, 0, "refs/heads/topic\n"),
            ErrorId::LocalRefsUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&remotes, 128, ""),
            ErrorId::RemoteConfigurationUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&remotes, 0, "origin fork\n"),
            ErrorId::RemoteConfigurationUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&reflog, 128, ""),
            ErrorId::ReflogUnobservable,
        ),
        (
            clean_host(&fixture, &project)?.answering(&reflog, 0, "not-a-commit\n"),
            ErrorId::ReflogUnobservable,
        ),
    ];

    for (host, expected) in cases {
        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
            .required_because("the failure is retained as an observation blocker")?;
        let error = gate::require_no_blockers(&assessment)
            .refused_because("an incomplete layer B inventory never reaches the confirmation")?;
        assert_eq!(error.first_id(), Some(expected));
    }
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
    let assessment = assess(&outside, &fixture, &project, DestructiveOperation::Destroy)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
fn a_sandbox_whose_git_lists_no_worktree_is_not_read_as_unsaved_work() -> Checked {
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
    let assessment = assess(&empty, &fixture, &project, DestructiveOperation::Rebuild)
        .required_because("a sandbox holding no worktree can be replaced")?;
    assert!(assessment.worktrees().is_empty());
    // 作業ツリーが1つも無くても、Sandboxへ書いたものは作り直しで失われる。
    assert_eq!(
        assessment.confirmable_losses(),
        [ConfirmableLoss::SandboxWritableLayer]
    );
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
                "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            0,
            "",
        );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("assess collects the blocker instead of failing outright")?;
    assert_eq!(
        assessment.blockers(),
        [Blocker::OriginUnreachable {
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
fn a_sandbox_without_the_shared_repository_is_not_read_as_unsaved_work() -> Checked {
    // 構築が途中で終わったSandboxには、この案件の作業が1件もない。worktreeが
    // 観測できないことを、失うものがある徴候として読まない。書き込み層の損失は
    // `a_sandbox_without_the_shared_repository_still_loses_its_writable_layer`が見る。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
            layout.bare_git_dir()
        ),
        1,
        "probed",
    );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
        .required_because("a sandbox that holds no repository can be replaced")?;
    assert!(assessment.worktrees().is_empty());
    Ok(())
}

#[test]
fn a_repository_probe_that_answers_a_status_the_inner_command_cannot_return_stops_the_run()
-> Checked {
    // 印がstdoutへ書かれていても、終了statusが`test -e`の答えでない値であれば、
    // repositoryの有無としては読めない。`sbx exec`自身の失敗を示す値も、`test`が
    // 返さない値も、どちらも観測できなかったこととして拒否する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let command = format!(
        "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    );

    for code in [126, 2] {
        let host = clean_host(&fixture, &project)?.answering(&command, code, "probed");

        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
            .required_because("the unreadable status is retained as an observation blocker")?;
        let error = gate::require_no_blockers(&assessment)
            .refused_because("a status the inner command cannot return is not absence")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreeInventoryUnobservable)
        );
    }
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("the unreadable HEAD is retained as a blocker")?;
    let error =
        gate::require_no_blockers(&assessment).refused_because("HEAD could not be resolved")?;
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
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
        ),
        126,
        "",
    );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("an origin that could not be observed is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
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
        "2 R. N... 100644 100644 100644 abc abc R100 new.txt\0old.txt\0",
    );

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
    fixture.create_workspace(&project)?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = InnerCommandSandbox::new().timing_out(&format!(
        "sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    ));

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
    // `BARE_GIT_DIR_PROBE`は、内側のshellが実際に走った場合だけstdoutへ印を書く。
    // stdoutが空のまま終わった場合、終了statusが何であれ、内側のcommandが答えた
    // 「不在」として読まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let command = format!(
        "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    );

    for code in [126, 2] {
        let host = clean_host(&fixture, &project)?.answering(&command, code, "");

        let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
            .required_because("an unstartable existence probe is retained as a blocker")?;
        let error = gate::require_no_blockers(&assessment)
            .refused_because("an unstartable existence probe is not absence")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreeInventoryUnobservable)
        );
    }
    Ok(())
}

#[test]
fn whether_head_is_attached_cannot_be_observed_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    fixture.create_workspace(&project)?;
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
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
    fixture.create_workspace(&project)?;
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

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Destroy)
        .required_because("unobserved upstream state is retained as a blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("whether an upstream is configured could not be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRefsUnobservable));
    Ok(())
}

#[test]
fn a_workspace_directory_missing_on_the_host_is_never_treated_as_no_repository() -> Checked {
    // hostのmount元が消えたSandboxへの`sbx exec`は、内側のcommandを起動できないまま
    // 終了statusだけを返す。その終了statusは「repositoryが無い」という答えと区別
    // できないため、host側を見ずに`sbx exec`だけへ頼ると同じ結論に落ちてしまう。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
        .required_because("the missing workspace is retained as an observation blocker")?;
    assert!(assessment.worktrees().is_empty());
    let error = gate::require_no_blockers(&assessment)
        .refused_because("a workspace that cannot be confirmed present is never read as empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxWorkspaceMissing));
    assert!(
        host.calls().is_empty(),
        "no exec into the sandbox is trusted before the workspace is confirmed present"
    );
    Ok(())
}

#[test]
fn a_workspace_directory_that_cannot_be_observed_is_never_treated_as_no_repository() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    fixture.workspace_is_unobservable(&project)?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
        .required_because("the unobservable workspace is retained as an observation blocker")?;
    let error = gate::require_no_blockers(&assessment)
        .refused_because("an unobservable workspace is never read as empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    Ok(())
}

#[test]
fn a_workspace_that_vanishes_between_the_host_check_and_the_repository_probe_is_never_treated_as_no_repository()
-> Checked {
    // hostのworkspace_existsが真を返した直後、次の`sbx exec`までの間にもmount元は
    // 消えうる。その`sbx exec`は内側のshellを起動できないまま終了status `1`だけを
    // 返し、これは`test -e`が答える「不在」の終了statusと同じ値である。終了statusの
    // 3値化だけでは、この2つを区別できない。stdoutに印が無ければ、「repositoryが
    // 無い」ではなく観測できなかったこととして拒否する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let command = format!(
        "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    );
    let host = clean_host(&fixture, &project)?.answering(&command, 1, "");

    let assessment = assess(&host, &fixture, &project, DestructiveOperation::Rebuild)
        .required_because("the unmarked answer is retained as an observation blocker")?;
    let error = gate::require_no_blockers(&assessment).refused_because(
        "a workspace that disappeared between the host check and the probe is never read as an empty repository",
    )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::WorktreeInventoryUnobservable)
    );
    Ok(())
}
