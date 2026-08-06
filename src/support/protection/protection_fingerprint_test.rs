use crate::project::SandboxName;
use crate::testing::outcome::Checked;
use crate::testing::project::project_id;

use std::collections::BTreeMap;

use super::super::{
    ConfirmableLoss, DestructiveOperation, Kind, Mode, OriginObservation, ProtectionAssessment,
    ProtectionBlocker, Reachability, WorktreeReport,
};
use super::ProtectionFingerprint;

/// 小文字16進64文字。表示や比較の実装先はまだ無く、この形式を守っていることを
/// ここからだけ確かめる。
impl ProtectionFingerprint {
    fn as_str(&self) -> &str {
        &self.hex
    }
}

fn sandbox() -> Checked<SandboxName> {
    Ok(SandboxName::derive(
        &project_id("example-org/example-repo")?.canonical(),
    ))
}

fn worktree(relative: &str) -> WorktreeReport {
    WorktreeReport {
        relative: relative.to_string(),
        kind: Kind::Managed,
        mode: Mode::Attached,
        head: "abc123".to_string(),
        branch: Some("main".to_string()),
        reachability: Reachability::Pushed {
            upstream: "refs/remotes/origin/main".to_string(),
        },
    }
}

fn observed(tip: &str) -> OriginObservation {
    OriginObservation::Observed {
        tips: BTreeMap::from([("refs/remotes/origin/main".to_string(), tip.to_string())]),
        reachable_from: BTreeMap::new(),
    }
}

fn assessment(
    worktrees: Vec<WorktreeReport>,
    blockers: Vec<ProtectionBlocker>,
    confirmable_losses: Vec<ConfirmableLoss>,
) -> Checked<ProtectionAssessment> {
    Ok(ProtectionAssessment::new(
        DestructiveOperation::Destroy,
        sandbox()?,
        worktrees,
        blockers,
        confirmable_losses,
        Some(observed("abc123")),
    ))
}

#[test]
fn collection_order_alone_does_not_change_the_fingerprint() -> Checked {
    let forward = assessment(
        vec![worktree("a"), worktree("b")],
        vec![
            ProtectionBlocker::TrackedChanges {
                worktree: "a".to_string(),
            },
            ProtectionBlocker::UntrackedPaths {
                worktree: "b".to_string(),
                paths: vec!["scratch.txt".to_string()],
            },
        ],
        vec![
            ConfirmableLoss::Tag {
                worktree: "a".to_string(),
                name: "v1".to_string(),
            },
            ConfirmableLoss::AdditionalRemote {
                worktree: "b".to_string(),
                name: "upstream".to_string(),
            },
        ],
    )?;
    let reversed = assessment(
        vec![worktree("b"), worktree("a")],
        vec![
            ProtectionBlocker::UntrackedPaths {
                worktree: "b".to_string(),
                paths: vec!["scratch.txt".to_string()],
            },
            ProtectionBlocker::TrackedChanges {
                worktree: "a".to_string(),
            },
        ],
        vec![
            ConfirmableLoss::AdditionalRemote {
                worktree: "b".to_string(),
                name: "upstream".to_string(),
            },
            ConfirmableLoss::Tag {
                worktree: "a".to_string(),
                name: "v1".to_string(),
            },
        ],
    )?;

    assert_eq!(
        ProtectionFingerprint::of(&forward),
        ProtectionFingerprint::of(&reversed),
        "the same state collected in a different order is the same fingerprint"
    );
    Ok(())
}

#[test]
fn any_difference_in_the_observed_state_changes_the_fingerprint() -> Checked {
    let base = assessment(vec![worktree("a")], Vec::new(), Vec::new())?;
    let different_head = assessment(
        vec![WorktreeReport {
            head: "def456".to_string(),
            ..worktree("a")
        }],
        Vec::new(),
        Vec::new(),
    )?;

    assert_ne!(
        ProtectionFingerprint::of(&base),
        ProtectionFingerprint::of(&different_head),
        "a commit that moved is a different state"
    );
    Ok(())
}

#[test]
fn a_change_in_the_origin_observation_alone_changes_the_fingerprint() -> Checked {
    let base = assessment(vec![worktree("a")], Vec::new(), Vec::new())?;
    let moved = ProtectionAssessment::new(
        DestructiveOperation::Destroy,
        sandbox()?,
        vec![worktree("a")],
        Vec::new(),
        Vec::new(),
        Some(observed("def456")),
    );

    assert_ne!(
        ProtectionFingerprint::of(&base),
        ProtectionFingerprint::of(&moved),
        "origin advancing to a different tip is a different state, even if no worktree changed"
    );
    Ok(())
}

#[test]
fn the_display_form_is_sixty_four_lowercase_hex_characters() -> Checked {
    let empty = assessment(Vec::new(), Vec::new(), Vec::new())?;
    let fingerprint = ProtectionFingerprint::of(&empty);
    let hex = fingerprint.as_str();

    assert_eq!(hex.len(), 64);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "the fingerprint is lowercase hex: {hex}"
    );
    Ok(())
}
