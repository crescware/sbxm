use crate::design::Cell;
use crate::msg;

/// `repair`が実際に変更する対象1件。
///
/// 包括的な「provisionする」ではなく、観測結果から導いた対象単位の操作だけを持つ。
/// `execute`はこの一覧と一致しない対象を変更しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairAction {
    /// 案件は既にfreshまたはreadyで、変更する対象がない。
    NoChange,
    /// 最初のmutationの前に、復旧intentを記録する。
    RecordIntent,
    /// labelが一致する既存imageを再利用する。
    ReuseImage,
    /// 一致するimageが無いため、固定したsnapshotからimageをbuildする。
    BuildImage,
    /// runtime idまで一致する既存Templateを再利用する。
    ReuseTemplate,
    /// 一致するTemplateが無いため、検証済みarchiveをloadする。
    LoadTemplate,
    /// 消えた中立workspace directoryを作り直す。
    RestoreWorkspace,
    /// Sandboxを作成する。
    CreateSandbox,
    /// 宣言file 1件を配置する。
    PlaceDeclaredFile { destination: String },
    /// `Sandbox内のGit` identityを設定する。
    ConfigureIdentity,
    /// credential helperを設定する。
    ConfigureCredentialHelper,
    /// bare repositoryを作成する。
    CreateBareRepository,
    /// managed worktree 1件を作成する。
    CreateWorktree { path: String },
    /// 全post-conditionを確認できたため、intentを消す。
    ClearIntent,
}

impl RepairAction {
    /// 表示用の1行。
    pub fn cell(&self) -> Cell {
        match self {
            RepairAction::NoChange => Cell::label(msg!("repair-action-no-change")),
            RepairAction::RecordIntent => Cell::label(msg!("repair-action-record-intent")),
            RepairAction::ReuseImage => Cell::label(msg!("repair-action-reuse-image")),
            RepairAction::BuildImage => Cell::label(msg!("repair-action-build-image")),
            RepairAction::ReuseTemplate => Cell::label(msg!("repair-action-reuse-template")),
            RepairAction::LoadTemplate => Cell::label(msg!("repair-action-load-template")),
            RepairAction::RestoreWorkspace => Cell::label(msg!("repair-action-restore-workspace")),
            RepairAction::CreateSandbox => Cell::label(msg!("repair-action-create-sandbox")),
            RepairAction::PlaceDeclaredFile { destination } => Cell::label(msg!(
                "repair-action-place-declared-file",
                destination = destination
            )),
            RepairAction::ConfigureIdentity => {
                Cell::label(msg!("repair-action-configure-identity"))
            }
            RepairAction::ConfigureCredentialHelper => {
                Cell::label(msg!("repair-action-configure-credential-helper"))
            }
            RepairAction::CreateBareRepository => {
                Cell::label(msg!("repair-action-create-bare-repository"))
            }
            RepairAction::CreateWorktree { path } => {
                Cell::label(msg!("repair-action-create-worktree", path = path))
            }
            RepairAction::ClearIntent => Cell::label(msg!("repair-action-clear-intent")),
        }
    }
}

#[cfg(test)]
#[path = "repair_action_test.rs"]
mod repair_action_test;
