use std::path::PathBuf;

use crate::metadata::CreationMode;
use crate::project::SandboxLayout;
use crate::support::Observed;
use crate::support::files::{PlacedFile, Placement};
use crate::support::provisioning::{Observation, ProvisioningState, WorktreeRow};
use crate::testing::metadata::attached;
use crate::testing::outcome::Checked;

use super::actions_for;
use crate::commands::repair::run::RepairAction;

fn incomplete_observation() -> Observation {
    let mut observation = Observation::new(
        ProvisioningState::Incomplete,
        "current".to_string(),
        "stored".to_string(),
        "target".to_string(),
    );
    observation.sandbox_present = true;
    observation.workspace_present = true;
    observation.identity_complete = true;
    observation.credential_helper = Observed::Matching;
    observation.repository_complete = true;
    observation.worktrees_complete = true;
    observation
}

#[test]
fn fresh_and_ready_projects_need_no_action() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    for state in [ProvisioningState::Fresh, ProvisioningState::Ready] {
        let observation =
            Observation::new(state, "a".to_string(), "a".to_string(), "a".to_string());
        assert_eq!(
            actions_for(&metadata, &observation, false),
            vec![RepairAction::NoChange]
        );
    }
    Ok(())
}

#[test]
fn a_missing_sandbox_and_intent_are_named_explicitly() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.sandbox_present = false;
    observation.workspace_present = false;

    let actions = actions_for(&metadata, &observation, false);
    assert!(actions.contains(&RepairAction::RecordIntent));
    assert!(actions.contains(&RepairAction::CreateSandbox));
    assert!(actions.contains(&RepairAction::RestoreWorkspace));
    Ok(())
}

#[test]
fn an_existing_intent_is_not_named_again() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let observation = incomplete_observation();
    let actions = actions_for(&metadata, &observation, true);
    assert!(!actions.contains(&RepairAction::RecordIntent));
    Ok(())
}

#[test]
fn a_matching_image_is_reused_rather_than_built() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.stored_image_matches = true;
    assert!(actions_for(&metadata, &observation, true).contains(&RepairAction::ReuseImage));

    let mut observation = incomplete_observation();
    observation.stored_image_matches = false;
    observation.current_image_matches = false;
    assert!(actions_for(&metadata, &observation, true).contains(&RepairAction::BuildImage));
    Ok(())
}

#[test]
fn a_present_template_is_reused_rather_than_loaded() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.stored_template_present = true;
    assert!(actions_for(&metadata, &observation, true).contains(&RepairAction::ReuseTemplate));

    let mut observation = incomplete_observation();
    observation.stored_template_present = false;
    observation.current_template_present = false;
    assert!(actions_for(&metadata, &observation, true).contains(&RepairAction::LoadTemplate));
    Ok(())
}

#[test]
fn a_declared_file_that_is_not_unchanged_is_named_by_its_destination() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.files = vec![
        PlacedFile {
            source: PathBuf::from("/home/user/.gitconfig"),
            destination: ".gitconfig".to_string(),
            placement: Placement::Unchanged,
        },
        PlacedFile {
            source: PathBuf::from("/home/user/.config/example.yaml"),
            destination: ".config/example.yaml".to_string(),
            placement: Placement::Placed,
        },
    ];

    let actions = actions_for(&metadata, &observation, true);
    assert!(actions.contains(&RepairAction::PlaceDeclaredFile {
        destination: ".config/example.yaml".to_string()
    }));
    assert!(!actions.contains(&RepairAction::PlaceDeclaredFile {
        destination: ".gitconfig".to_string()
    }));
    Ok(())
}

#[test]
fn incomplete_identity_credential_and_repository_are_named() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.identity_complete = false;
    observation.credential_helper = Observed::Missing;
    observation.repository_complete = false;

    let actions = actions_for(&metadata, &observation, true);
    assert!(actions.contains(&RepairAction::ConfigureIdentity));
    assert!(actions.contains(&RepairAction::ConfigureCredentialHelper));
    assert!(actions.contains(&RepairAction::CreateBareRepository));
    Ok(())
}

#[test]
fn a_missing_managed_worktree_is_named_by_its_path() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = incomplete_observation();
    observation.worktrees_complete = false;
    observation.worktrees = Vec::new();

    let actions = actions_for(&metadata, &observation, true);
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, RepairAction::CreateWorktree { .. }))
    );
    assert!(actions.contains(&RepairAction::ClearIntent));
    Ok(())
}

#[test]
fn an_already_present_managed_worktree_is_not_named_again() -> Checked {
    let mut metadata = attached("example-org", "example-repo")?;
    metadata.provisioning.requested_worktrees = 2;
    let layout = SandboxLayout::new(metadata.canonical_id());
    let names = layout.worktree_names(metadata.provisioning.requested_worktrees);
    let (present, missing) = (names[0].clone(), names[1].clone());

    let mut observation = incomplete_observation();
    observation.worktrees_complete = false;
    observation.worktrees = vec![WorktreeRow {
        path: present.clone(),
        created_from: "main".to_string(),
        head: "a1b2c3d".to_string(),
        mode: CreationMode::Attached,
    }];

    let actions = actions_for(&metadata, &observation, true);
    assert!(!actions.contains(&RepairAction::CreateWorktree { path: present }));
    assert!(actions.contains(&RepairAction::CreateWorktree { path: missing }));
    Ok(())
}
