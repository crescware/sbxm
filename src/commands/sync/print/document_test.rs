use std::path::PathBuf;

use crate::design::{RenderingPolicy, Ui};
use crate::i18n::Locale;
use crate::support::host_sync::{ReflectResult, Reflected};

use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::commands::sync::{SentChange, SyncOutput};

fn rendered(output: &SyncOutput, locale: Locale) -> Checked<String> {
    let mut written: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(
            locale,
            RenderingPolicy::plain(),
            &mut written,
            std::io::sink(),
        );
        ui.stdout(&document(output, locale));
    }
    String::from_utf8(written).required_because("UTF-8")
}

fn output(reflected: Option<Vec<Reflected>>, sent: Vec<SentChange>) -> SyncOutput {
    SyncOutput {
        project: "local/app".to_string(),
        repository: PathBuf::from("/Users/example/code/app/.git"),
        namespace: "sbxm-local-app-0123456789ab".to_string(),
        reflected,
        sent,
    }
}

fn reflected(reference: &str, result: ReflectResult) -> Reflected {
    Reflected {
        reference: reference.to_string(),
        result,
    }
}

#[test]
fn both_directions_are_listed_under_their_own_headings() -> Checked {
    let text = rendered(
        &output(
            Some(vec![
                reflected("refs/heads/topic", ReflectResult::Created),
                reflected("refs/heads/main", ReflectResult::Updated),
            ]),
            vec![
                SentChange::Created {
                    reference: "refs/remotes/origin/topic".to_string(),
                },
                SentChange::Updated {
                    reference: "refs/remotes/origin/main".to_string(),
                },
                SentChange::Removed {
                    reference: "refs/remotes/origin/old".to_string(),
                },
            ],
        ),
        Locale::En,
    )?;
    assert!(
        text.starts_with("\u{2713} Synced local/app with /Users/example/code/app/.git."),
        "{text}"
    );
    for expected in [
        "Host repository",
        "refs/heads/topic",
        "created",
        "Sandbox origin",
        "refs/remotes/origin/main",
        "refs/remotes/origin/old",
        "removed",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    // gitが断ったrefが無ければ、残した場所は示さない。hostとSandboxのbranchが揃えば、
    // Sandboxの中でする取り込みも示さない。
    assert!(!text.contains("refs/sbx/"), "{text}");
    assert!(!text.contains("origin/<branch>"), "{text}");
    Ok(())
}

#[test]
fn refs_git_left_as_they_were_point_at_where_their_commits_are_kept() -> Checked {
    let text = rendered(
        &output(
            Some(vec![
                reflected("refs/heads/back", ReflectResult::Behind),
                reflected("refs/heads/fork", ReflectResult::Diverged),
                reflected("refs/heads/rebasing", ReflectResult::CheckedOut),
                reflected(
                    "refs/heads/main",
                    ReflectResult::LocalChanges {
                        worktree: PathBuf::from("/Users/example/code/app"),
                        paths: vec!["src/lib.rs".to_string(), "README.md".to_string()],
                    },
                ),
                reflected("refs/tags/v1", ReflectResult::Exists),
                reflected(
                    "refs/heads/guarded",
                    ReflectResult::Refused {
                        reason: "pre-receive hook declined".to_string(),
                    },
                ),
            ]),
            Vec::new(),
        ),
        Locale::En,
    )?;
    for expected in [
        "behind",
        "diverged",
        "checked-out",
        "local-changes",
        "exists",
        "refused",
        "refs/sbx/sbxm-local-app-0123456789ab",
        "Git refused refs/heads/guarded: pre-receive hook declined",
        "refs/heads/main is checked out in /Users/example/code/app, where uncommitted changes to src/lib.rs, README.md overlap",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    Ok(())
}

#[test]
fn a_sync_git_left_any_ref_of_is_not_said_to_be_synced() -> Checked {
    // 終了status 1と同じく、hostの側でもSandboxのoriginの側でも、断られたものがあれば
    // 同期したとは言わない。遅れているだけのbranchは数えない。
    let refused_on_the_host = output(
        Some(vec![
            reflected("refs/heads/topic", ReflectResult::Updated),
            reflected("refs/heads/main", ReflectResult::CheckedOut),
        ]),
        Vec::new(),
    );
    let refused_in_the_sandbox = output(
        Some(vec![reflected("refs/heads/topic", ReflectResult::Updated)]),
        vec![SentChange::Refused {
            reference: "refs/tags/v1".to_string(),
            reason: "already exists".to_string(),
        }],
    );
    for refused in [refused_on_the_host, refused_in_the_sandbox] {
        let text = rendered(&refused, Locale::En)?;
        assert!(
            text.starts_with(
                "! Warning: local/app is not fully in sync with /Users/example/code/app/.git"
            ),
            "{text}"
        );
        assert!(!text.contains("Synced"), "{text}");
    }
    let behind = rendered(
        &output(
            Some(vec![reflected("refs/heads/back", ReflectResult::Behind)]),
            Vec::new(),
        ),
        Locale::En,
    )?;
    assert!(behind.starts_with("\u{2713} Synced local/app"), "{behind}");
    Ok(())
}

#[test]
fn a_branch_that_is_only_behind_loses_nothing_and_is_named_to_be_taken_in() -> Checked {
    let text = rendered(
        &output(
            Some(vec![
                reflected("refs/heads/back", ReflectResult::Behind),
                reflected("refs/heads/main", ReflectResult::Updated),
                reflected("refs/heads/old", ReflectResult::Behind),
            ]),
            Vec::new(),
        ),
        Locale::En,
    )?;
    assert!(text.contains("behind"), "{text}");
    assert!(!text.contains("refs/sbx/"), "{text}");
    assert!(
        text.contains("These sandbox branches lack commits the host has: back, old."),
        "{text}"
    );
    // hostに届けるものが残るのは、分かれたbranchだけである。
    assert!(!text.contains("sync again"), "{text}");
    Ok(())
}

#[test]
fn a_branch_that_diverged_is_named_to_be_merged_and_synced_again() -> Checked {
    let text = rendered(
        &output(
            Some(vec![
                reflected("refs/heads/back", ReflectResult::Behind),
                reflected("refs/heads/fork", ReflectResult::Diverged),
            ]),
            Vec::new(),
        ),
        Locale::Ja,
    )?;
    for expected in [
        "次のSandboxのbranchには、hostにあるcommitが足りません: back。",
        "次のSandboxのbranchとhostは、それぞれ相手に無いcommitを持っています: fork。",
        "もう一度同期してhostへ届けてください",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    Ok(())
}

#[test]
fn a_tag_the_sandbox_kept_is_listed_with_why_git_refused_it() -> Checked {
    let text = rendered(
        &output(
            None,
            vec![SentChange::Refused {
                reference: "refs/tags/v1".to_string(),
                reason: "already exists".to_string(),
            }],
        ),
        Locale::En,
    )?;
    for expected in [
        "refs/tags/v1",
        "refused",
        "Git refused refs/tags/v1: already exists",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    Ok(())
}

#[test]
fn a_sync_that_changed_nothing_says_so() -> Checked {
    for reflected in [None, Some(Vec::new())] {
        let text = rendered(&output(reflected, Vec::new()), Locale::Ja)?;
        assert!(text.contains("すでに同期しています"), "{text}");
    }
    Ok(())
}

#[test]
fn the_results_are_explained_in_a_translated_legend() -> Checked {
    let text = rendered(
        &output(
            Some(vec![reflected("refs/heads/fork", ReflectResult::Diverged)]),
            Vec::new(),
        ),
        Locale::Ja,
    )?;
    assert!(
        text.contains("hostとSandboxで、それぞれ相手に無いcommitを持つbranchです"),
        "{text}"
    );
    Ok(())
}
