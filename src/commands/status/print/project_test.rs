//! project scopeの`status`の出力。
//!
//! 表と診断は行き先が別であり、入れ替わってもexit codeでは気付けない。2つのstreamを
//! 別々に受け取り、どちらに何が出たかを確かめる。

use crate::design::RenderingPolicy;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::i18n::Locale;

use crate::compatibility::RootDiskUsage;
use crate::support::disk::DiskObservation;
use crate::support::protection::{Reachability, UnobservableReason};

use crate::commands::status::project::{Item, Value, WorktreeRow};

use crate::testing::outcome::{Checked, Required};

use super::*;

struct Printed {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn print(status: &ProjectStatus) -> Checked<Printed> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            &mut stderr,
        );
        project(&mut ui, status)
    };
    Ok(Printed {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
    })
}

/// 問題を1件も持たない診断結果。
fn healthy() -> ProjectStatus {
    ProjectStatus {
        project: "example-org/example-repo".to_string(),
        items: vec![Item {
            item: "status-item-bare-repository",
            value: Value::Ready,
        }],
        worktrees: vec![WorktreeRow {
            path: "example-repo.tree-0".to_string(),
            kind: "managed",
            mode: Value::Attached,
            state: Value::Clean,
            remote: Reachability::Pushed {
                upstream: "refs/remotes/origin/main".to_string(),
            },
        }],
        disk: DiskObservation::Observed(RootDiskUsage {
            free_kib: 4_898_320,
            usable_kib: 19_401_296,
            capacity_percent: 75,
        }),
        diagnostics: Vec::new(),
    }
}

fn unusable_repository() -> Diagnostic {
    Diagnostic::new(
        ErrorId::SandboxRepositoryUnusable,
        crate::msg!("error-sandbox-repository-unusable"),
    )
}

#[test]
fn a_project_without_a_problem_succeeds_and_leaves_stderr_untouched() -> Checked {
    let printed = print(&healthy())?;

    assert_eq!(printed.code, ExitCode::Success);
    assert!(printed.stderr.is_empty(), "{:?}", printed.stderr);
    // 表そのものが結論であるため、健全な案件でも行は出る。
    assert!(printed.stdout.contains("PROJECT"), "{:?}", printed.stdout);
    assert!(
        printed.stdout.contains("example-org/example-repo"),
        "{:?}",
        printed.stdout
    );
    assert!(
        printed.stdout.contains(Value::Clean.as_str()),
        "{:?}",
        printed.stdout
    );
    assert!(printed.stdout.contains("REMOTE"), "{:?}", printed.stdout);
    assert!(printed.stdout.contains("pushed"), "{:?}", printed.stdout);
    Ok(())
}

#[test]
fn every_remote_state_is_printed_separately_from_worktree_state() -> Checked {
    let remotes = [
        Reachability::Pushed {
            upstream: "refs/remotes/origin/main".to_string(),
        },
        Reachability::Reachable {
            origins: vec!["refs/remotes/origin/release".to_string()],
        },
        Reachability::Unreachable,
        Reachability::Unobservable {
            reason: UnobservableReason::ReadOnlyDataInsufficient,
        },
    ];
    let mut status = healthy();
    status.worktrees = remotes
        .into_iter()
        .enumerate()
        .map(|(index, remote)| WorktreeRow {
            path: format!("worktree-{index}"),
            kind: "managed",
            mode: Value::Attached,
            state: Value::Clean,
            remote,
        })
        .collect();

    let printed = print(&status)?;

    for value in [
        "pushed",
        "reachable",
        "unreachable",
        "unobservable(read-only-data-insufficient)",
    ] {
        assert!(
            printed.stdout.contains(value),
            "{value}: {:?}",
            printed.stdout
        );
    }
    Ok(())
}

#[test]
fn the_disk_section_always_appears_observed_or_not() -> Checked {
    let observed = print(&healthy())?;
    assert!(observed.stdout.contains("DISK"), "{:?}", observed.stdout);
    assert!(observed.stdout.contains("75"), "{:?}", observed.stdout);

    for (disk, reason) in [
        (DiskObservation::NotObservedStopped, "did not start it"),
        (DiskObservation::NotObservedNotCreated, "does not exist yet"),
        (
            DiskObservation::NotObservedMismatch,
            "could not be determined",
        ),
        (DiskObservation::CommandMissing, "is not available"),
        (DiskObservation::ParseFailed, "could not be read"),
    ] {
        let mut status = healthy();
        status.disk = disk;
        let printed = print(&status)?;
        assert_eq!(printed.code, ExitCode::Success, "{disk:?} stays healthy");
        assert!(
            printed.stdout.contains("DISK"),
            "{disk:?}: {:?}",
            printed.stdout
        );
        assert!(
            printed.stdout.contains(reason),
            "{disk:?}: {:?}",
            printed.stdout
        );
    }
    Ok(())
}

#[test]
fn a_diagnosed_project_fails_and_every_diagnostic_is_written_on_its_own() -> Checked {
    let mut status = healthy();
    status.diagnostics.push(unusable_repository());
    status.diagnostics.push(unusable_repository());

    let printed = print(&status)?;

    assert_eq!(printed.code, ExitCode::Failure);
    assert_eq!(
        printed.stderr.matches("\u{d7} error:").count(),
        2,
        "{:?}",
        printed.stderr
    );
    // 診断が出ても、読めた項目の表は隠れない。
    assert!(printed.stdout.contains("PROJECT"), "{:?}", printed.stdout);
    assert!(!printed.stdout.contains("error:"), "{:?}", printed.stdout);
    Ok(())
}
