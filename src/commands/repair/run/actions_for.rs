use std::collections::BTreeSet;

use crate::metadata::ProjectMetadata;
use crate::project::SandboxLayout;
use crate::support::files::Placement;
use crate::support::provisioning::{Observation, ProvisioningState};

use super::RepairAction;

/// 観測結果から、実際に変更する対象だけを1対1で導く。
///
/// `Fresh`・`Ready`は変更対象を持たない。それ以外は、観測結果がまだ満たしていない
/// artifactの分だけ、対象単位のactionを積む。`prepare`はこれを一度計算して表示し、
/// `execute`はmutationの直前にこの関数を再実行して、表示した一覧と一致することを
/// 確かめてから進む。
pub(super) fn actions_for(
    metadata: &ProjectMetadata,
    observation: &Observation,
    has_intent: bool,
) -> Vec<RepairAction> {
    if matches!(
        observation.state,
        ProvisioningState::Fresh | ProvisioningState::Ready
    ) {
        return vec![RepairAction::NoChange];
    }

    let mut actions = Vec::new();
    if !has_intent {
        actions.push(RepairAction::RecordIntent);
    }
    if !observation.sandbox.is_matching() {
        actions.push(RepairAction::CreateSandbox);
    }
    if observation.stored_image_matches || observation.current_image_matches {
        actions.push(RepairAction::ReuseImage);
    } else {
        actions.push(RepairAction::BuildImage);
    }
    if observation.stored_template_present || observation.current_template_present {
        actions.push(RepairAction::ReuseTemplate);
    } else {
        actions.push(RepairAction::LoadTemplate);
    }
    if !observation.workspace.is_matching() {
        actions.push(RepairAction::RestoreWorkspace);
    }
    if observation.interior_is_unobservable() {
        // 停止中Sandboxは、最初の内部検査に使うsbx execが暗黙に起動する。内部の
        // 個別artifactを欠落と推測せず、未観測のため全工程を実行する事実を示す。
        actions.push(RepairAction::StartSandbox);
        actions.push(RepairAction::ProvisionInterior);
    } else {
        for file in &observation.files {
            if file.placement != Placement::Unchanged {
                actions.push(RepairAction::PlaceDeclaredFile {
                    destination: file.destination.clone(),
                });
            }
        }
        if !observation.identity.is_matching() {
            actions.push(RepairAction::ConfigureIdentity);
        }
        if !observation.credential_helper.is_matching() {
            actions.push(RepairAction::ConfigureCredentialHelper);
        }
        if !observation.repository.is_matching() {
            actions.push(RepairAction::CreateBareRepository);
        }
        if !observation.worktrees_present.is_matching() {
            let layout = SandboxLayout::new(metadata.canonical_id());
            let existing: BTreeSet<&str> = observation
                .worktrees
                .iter()
                .map(|worktree| worktree.path.as_str())
                .collect();
            for name in layout.worktree_names(metadata.provisioning.requested_worktrees) {
                if !existing.contains(name.as_str()) {
                    actions.push(RepairAction::CreateWorktree { path: name });
                }
            }
        }
    }
    actions.push(RepairAction::ClearIntent);
    actions
}

#[cfg(test)]
#[path = "actions_for_test.rs"]
mod actions_for_test;
