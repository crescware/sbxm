use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::support::Observed;
use crate::testing::outcome::{Checked, Refused};

use super::{Observation, ProvisioningState};

fn observation() -> Observation {
    Observation::new(
        ProvisioningState::Fresh,
        "current".to_string(),
        "stored".to_string(),
        "target".to_string(),
    )
}

/// 目標構成をすべて観測できた案件。
fn complete() -> Observation {
    let mut observation = observation();
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

/// 中を読めなかった停止中Sandbox。
fn not_observed() -> Observed {
    Observed::Unobservable {
        evidence: "stopped".to_string(),
    }
}

fn refusal() -> Error {
    Error::single(Diagnostic::new(
        ErrorId::SandboxWorkspaceNotEmpty,
        msg!("error-sandbox-workspace-not-empty"),
    ))
}

#[test]
fn a_project_without_any_artifact_is_fresh() {
    assert_eq!(observation().classify(false), ProvisioningState::Fresh);
}

#[test]
fn a_saved_intent_outranks_every_other_observation() {
    // 成果物が完成して見えても、完了確認とclearが済むまではpendingのままにする。
    assert_eq!(complete().classify(true), ProvisioningState::Pending);
    assert_eq!(observation().classify(true), ProvisioningState::Pending);
}

#[test]
fn a_fully_observed_project_is_ready() {
    assert_eq!(complete().classify(false), ProvisioningState::Ready);
}

#[test]
fn a_sandbox_whose_inside_was_not_read_is_not_incomplete() {
    // 停止中のSandboxは中を読めない。読まなかったことを欠落として扱わない。
    let mut observation = complete();
    observation.files_placed = not_observed();
    observation.identity = not_observed();
    observation.repository = not_observed();
    observation.worktrees_present = not_observed();

    assert_eq!(
        observation.classify(false),
        ProvisioningState::Unobservable,
        "an unread sandbox interior is not a missing one"
    );
    assert!(!observation.is_complete());
    assert!(!observation.has_definite_gap());
}

#[test]
fn an_observed_gap_outranks_what_could_not_be_observed() {
    // workspaceが無いことは観測できている。中を読めなかった項目があっても、
    // 復旧できる欠落として扱う。
    let mut observation = complete();
    observation.workspace = Observed::Missing;
    observation.repository = not_observed();

    assert!(observation.has_definite_gap());
    assert_eq!(observation.classify(false), ProvisioningState::Incomplete);
}

#[test]
fn a_mismatching_artifact_is_a_gap_rather_than_a_missing_observation() {
    let mut observation = complete();
    observation.repository = Observed::Mismatch {
        evidence: "repository-unusable".to_string(),
    };

    assert!(observation.has_definite_gap());
    assert_eq!(observation.classify(false), ProvisioningState::Incomplete);
}

#[test]
fn an_observation_without_refusals_lets_a_mutation_proceed() -> Checked {
    complete().require_safe()?;
    Ok(())
}

#[test]
fn a_recorded_refusal_stops_a_mutation_before_it_starts() -> Checked {
    let mut observation = complete();
    observation.block_all(vec![refusal()]);

    let error = observation
        .require_safe()
        .refused_because("an observation that could not be proven safe blocks mutations")?;
    assert!(error.contains_id(ErrorId::SandboxWorkspaceNotEmpty));
    Ok(())
}
