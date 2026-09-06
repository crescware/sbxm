use crate::design::Cell;
use crate::testing::outcome::Checked;

use super::RepairAction;

/// 全variantが翻訳可能な1行を持つことを確かめる。
#[test]
fn every_action_renders_a_label_cell() -> Checked {
    let actions = [
        RepairAction::NoChange,
        RepairAction::RecordIntent,
        RepairAction::ReuseImage,
        RepairAction::BuildImage,
        RepairAction::ReuseTemplate,
        RepairAction::LoadTemplate,
        RepairAction::RestoreWorkspace,
        RepairAction::CreateSandbox,
        RepairAction::PlaceDeclaredFile {
            destination: ".config/example.yaml".to_string(),
        },
        RepairAction::ConfigureIdentity,
        RepairAction::ConfigureCredentialHelper,
        RepairAction::CreateBareRepository,
        RepairAction::CreateWorktree {
            path: "example-repo.tree-0".to_string(),
        },
        RepairAction::ClearIntent,
    ];
    for action in actions {
        let Cell::Label(message) = action.cell() else {
            return Err(crate::testing::outcome::Unmet::new(format!(
                "{action:?} does not render as a label"
            )));
        };
        assert!(!message.id.is_empty(), "{action:?}");
    }
    Ok(())
}
