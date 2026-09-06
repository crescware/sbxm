use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::metadata::{ProjectMetadata, RebuildIntent};
use crate::msg;
use crate::support::Observed;
use crate::testing::metadata::{OTHER_DIGEST, attached};
use crate::testing::outcome::Checked;
use crate::testing::value::DIGEST;

use super::{NextAction, Observation, ProvisioningState};

/// 世代が一致し、artifactを1つも観測していない出発点。
fn observation(state: ProvisioningState) -> Observation {
    let mut observation = Observation::new(
        state,
        DIGEST.to_string(),
        DIGEST.to_string(),
        DIGEST.to_string(),
    );
    observation.state = state;
    observation
}

/// 目標構成をすべて観測できた案件。
fn ready() -> Observation {
    let mut observation = observation(ProvisioningState::Ready);
    observation.sandbox = Observed::Matching;
    observation.workspace = Observed::Matching;
    observation.files_placed = Observed::Matching;
    observation.identity = Observed::Matching;
    observation.tools = Observed::Matching;
    observation.credentials = Observed::Matching;
    observation.secret = Observed::Matching;
    observation.credential_helper = Observed::Matching;
    observation.repository = Observed::Matching;
    observation.worktrees_present = Observed::Matching;
    observation
}

fn rebuilding(metadata: &ProjectMetadata) -> ProjectMetadata {
    ProjectMetadata {
        rebuild: Some(RebuildIntent {
            target_dockerfile_sha256: OTHER_DIGEST.to_string(),
            previous_dockerfile_sha256: DIGEST.to_string(),
        }),
        ..metadata.clone()
    }
}

#[test]
fn an_unfinished_first_provisioning_is_recovered_before_anything_else() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    assert_eq!(
        NextAction::decide(&metadata, &observation(ProvisioningState::Pending)),
        Some(NextAction::RepairPending)
    );
    assert_eq!(
        NextAction::decide(&metadata, &observation(ProvisioningState::Incomplete)),
        Some(NextAction::RepairIncomplete)
    );
    Ok(())
}

#[test]
fn a_project_that_needs_nothing_is_not_given_a_command() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    assert_eq!(NextAction::decide(&metadata, &ready()), None);
    assert_eq!(
        NextAction::decide(&metadata, &observation(ProvisioningState::Fresh)),
        None,
        "a project that was never built has nothing to recover or to update"
    );
    Ok(())
}

#[test]
fn an_unfinished_generation_change_is_completed_rather_than_restarted() -> Checked {
    let metadata = rebuilding(&attached("example-org", "example-repo")?);
    assert_eq!(
        NextAction::decide(&metadata, &ready()),
        Some(NextAction::RebuildPending)
    );
    Ok(())
}

#[test]
fn a_changed_dockerfile_is_applied_as_a_new_generation() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut changed = ready();
    changed.current_generation = OTHER_DIGEST.to_string();

    assert_eq!(
        NextAction::decide(&metadata, &changed),
        Some(NextAction::RebuildChanged)
    );
    // まだ構築していない案件のDockerfileは、適用済み世代との差分ではない。
    let mut fresh = observation(ProvisioningState::Fresh);
    fresh.current_generation = OTHER_DIGEST.to_string();
    assert_eq!(NextAction::decide(&metadata, &fresh), None);
    Ok(())
}

#[test]
fn a_project_that_needs_recovery_is_not_also_told_to_rebuild() -> Checked {
    // 復旧と世代交代の両方に関係する事実があっても、今すぐ実行するcommandは1つだけ示す。
    let metadata = rebuilding(&attached("example-org", "example-repo")?);
    let mut observation = observation(ProvisioningState::Incomplete);
    observation.current_generation = OTHER_DIGEST.to_string();

    let action = NextAction::decide(&metadata, &observation);
    assert_eq!(action, Some(NextAction::RepairIncomplete));
    assert!(
        action.is_some_and(NextAction::leaves_generation_behind),
        "the caller can explain that a rebuild may still follow"
    );
    Ok(())
}

#[test]
fn a_project_whose_state_could_not_be_observed_is_not_given_a_command() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    assert_eq!(
        NextAction::decide(&metadata, &observation(ProvisioningState::Unobservable)),
        None
    );

    // 安全と確認できない事実があれば、状態が復旧可能に見えてもcommandを出さない。
    let mut unsafe_observation = observation(ProvisioningState::Incomplete);
    unsafe_observation.block_all(vec![Error::single(Diagnostic::new(
        ErrorId::SandboxUnusable,
        msg!("error-sandbox-unusable"),
    ))]);
    assert_eq!(NextAction::decide(&metadata, &unsafe_observation), None);
    Ok(())
}

#[test]
fn every_action_names_its_command_reason_and_exit_meaning() {
    for (action, command, blocking) in [
        (NextAction::RepairPending, "sbxm repair owner/repo", true),
        (NextAction::RepairIncomplete, "sbxm repair owner/repo", true),
        (NextAction::RebuildPending, "sbxm rebuild owner/repo", true),
        (NextAction::RebuildChanged, "sbxm rebuild owner/repo", false),
    ] {
        assert_eq!(action.command("owner/repo"), command);
        assert_eq!(action.is_blocking(), blocking, "{action:?}");
        assert!(action.reason_id().starts_with("guidance-next-"));
    }
    assert!(!NextAction::RebuildChanged.leaves_generation_behind());
    assert!(!NextAction::RebuildPending.leaves_generation_behind());
}
