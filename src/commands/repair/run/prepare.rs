use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, Field, Inline, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxName};
use crate::support::files::Placement;
use crate::support::provisioning::{self, Observation, ProvisioningState};
use crate::support::select::ProjectPrompt;

use super::actions_for::actions_for;
use super::{Prepared, RepairPlan};

/// 対象を決め、repair前の観測と変更範囲を固定する。
pub fn prepare(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Prepared> {
    let locked =
        crate::support::select::one(location, requested, &msg!("select-repair-heading"), prompt)?
            .lock()?;
    crate::support::generation::require_no_rebuild(&locked.metadata)?;
    if let Some(intent) = &locked.metadata.initial_provisioning {
        provisioning::validate_intent(intent, config, &locked.metadata.display_id())?;
    }
    let first = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    let name = locked.metadata.sandbox_name();

    if matches!(
        first.state,
        ProvisioningState::Fresh | ProvisioningState::Ready
    ) {
        let target = if first.state == ProvisioningState::Fresh {
            &first.current_generation
        } else {
            &first.target_generation
        };
        let plan = plan(&locked.metadata, &first, target, false);
        return Ok(Prepared {
            paths: locked.paths.clone(),
            locked,
            observation: first,
            target: plan.target_generation.clone(),
            preconditions: None,
            session_lease: None,
            warnings: Vec::new(),
            plan,
        });
    }

    let target = target_generation(&first, &locked.metadata)?;
    let session_lease = locked.acquire_exclusive_session_lease()?;

    // lease取得後にもう一度読む。別workflowが先に成果物を進めていた場合は、最初の
    // repair計画をそのまま適用しない。
    let second = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    let second_target = target_generation(&second, &locked.metadata)?;
    if second.state != first.state || second_target != target {
        return Err(state_changed(&locked.metadata, first.state, second.state));
    }
    if let Some(intent) = &locked.metadata.initial_provisioning {
        provisioning::validate_intent(intent, config, &locked.metadata.display_id())?;
    }
    let preconditions = provisioning::verify_external_preconditions(host, &name)?;
    let plan = plan(
        &locked.metadata,
        &second,
        &target,
        locked.metadata.initial_provisioning.is_some(),
    );
    Ok(Prepared {
        paths: locked.paths.clone(),
        locked,
        observation: second,
        target,
        preconditions: Some(preconditions),
        session_lease: Some(session_lease),
        warnings: Vec::new(),
        plan,
    })
}

fn target_generation(observation: &Observation, metadata: &ProjectMetadata) -> Result<String> {
    if metadata.initial_provisioning.is_some() {
        if observation.current_generation != observation.target_generation
            && !observation.stored_image_matches
        {
            return Err(generation_missing(observation, metadata));
        }
        return Ok(observation.target_generation.clone());
    }
    if observation.current_generation == observation.stored_generation {
        return Ok(observation.stored_generation.clone());
    }
    match (
        observation.stored_image_matches,
        observation.current_image_matches,
    ) {
        (true, false) => Ok(observation.stored_generation.clone()),
        (false, true) => Ok(observation.current_generation.clone()),
        _ => Err(generation_missing(observation, metadata)),
    }
}

fn generation_missing(observation: &Observation, metadata: &ProjectMetadata) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningGenerationMissing,
            msg!(
                "error-initial-provisioning-generation-missing",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::value(short_hex(&observation.target_generation)))
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-generation-ambiguous"
        )))
        .remediation(Remediation::text(msg!(
            "remediation-initial-provisioning-generation-missing"
        ))),
    )
}

fn state_changed(
    metadata: &ProjectMetadata,
    before: ProvisioningState,
    after: ProvisioningState,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningStateChanged,
            msg!(
                "error-initial-provisioning-state-changed",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-state-changed",
            before = before,
            after = after
        )))
        .remediation(msg!("remediation-run-repair")),
    )
}

fn plan(
    metadata: &ProjectMetadata,
    observation: &Observation,
    target: &str,
    has_intent: bool,
) -> RepairPlan {
    RepairPlan {
        project: metadata.display_id(),
        sandbox: SandboxName::derive(metadata.canonical_id()).to_string(),
        state: observation.state,
        target_generation: target.to_string(),
        observations: observations_for(observation),
        actions: actions_for(metadata, observation, has_intent),
    }
}

/// artifactごとの観測結果を、変更対象と分けて表示するための一覧。
fn observations_for(observation: &Observation) -> Vec<Field> {
    let mut fields = vec![
        artifact_field("repair-observation-sandbox", observation.sandbox_present),
        artifact_field(
            "repair-observation-workspace",
            observation.workspace_present,
        ),
    ];
    for file in &observation.files {
        fields.push(Field::new(
            msg!(
                "repair-observation-declared-file",
                destination = file.destination.clone()
            ),
            Inline::important(matching_or_missing(file.placement == Placement::Unchanged)),
        ));
    }
    fields.push(artifact_field(
        "repair-observation-identity",
        observation.identity_complete,
    ));
    fields.push(artifact_field(
        "repair-observation-credential-helper",
        observation.credential_helper.is_matching(),
    ));
    fields.push(artifact_field(
        "repair-observation-repository",
        observation.repository_complete,
    ));
    fields.push(artifact_field(
        "repair-observation-worktrees",
        observation.worktrees_complete,
    ));
    fields
}

fn artifact_field(label: &'static str, matching: bool) -> Field {
    Field::new(
        msg!(label),
        Inline::important(matching_or_missing(matching)),
    )
}

/// 翻訳しない安定した表記。
fn matching_or_missing(matching: bool) -> &'static str {
    if matching { "matching" } else { "missing" }
}
