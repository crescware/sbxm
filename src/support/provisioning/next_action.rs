use crate::metadata::ProjectMetadata;

use super::{Observation, ProvisioningState};

/// 観測結果から導いた、利用者が今すぐ実行できる1手。
///
/// `repair`と`rebuild`は向かうgenerationが違う。`repair`は固定済みのtarget generationへ
/// 戻し、`rebuild`は現在のDockerfileが表す新しいgenerationへ進める。どちらを選んだかを
/// 理由まで含めて型で持ち、案内する側に推測させない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextAction {
    /// 初回構築のintentが残っている。固定済みgenerationへ戻す。
    RepairPending,
    /// intentは無いが、初回構築の成果物が欠けていることを観測した。
    RepairIncomplete,
    /// 開始済みの世代交代が残っている。同じ世代交代を完了する。
    RebuildPending,
    /// 案件は使えるが、現在のDockerfileが適用済みgenerationと違う。
    RebuildChanged,
}

impl NextAction {
    /// 観測結果から、今すぐ実行できる1手を1つだけ決める唯一の規則。
    ///
    /// 相反するcommandを並べない。安全と証明できない観測と、完了を観測できない案件では、
    /// 実行できると言えないためcommandを出さない。復旧が要る案件では、Dockerfileが
    /// 変わっていても`repair`だけを示す。固定済みgenerationへ戻したあとにもう一度
    /// 観測すれば、必要に応じて`rebuild`が案内される。
    pub fn decide(metadata: &ProjectMetadata, observation: &Observation) -> Option<NextAction> {
        // 安全と確認できない事実が1つでもあれば、どのcommandが安全かを決められない。
        if observation.require_safe().is_err() {
            return None;
        }
        // custom secretがSandboxへ適用されていない場合、repairはplaceholder検査で必ず
        // 拒否される。実際の復旧手段はsecret診断が示すSandbox削除であり、ここから
        // 成功しないrepairを重ねて案内しない。
        if observation.sandbox.is_matching() && observation.secret.is_missing() {
            return None;
        }

        match observation.state {
            ProvisioningState::Pending => return Some(NextAction::RepairPending),
            ProvisioningState::Incomplete => return Some(NextAction::RepairIncomplete),
            // 中を観測できない案件へ、欠けているとも揃っているとも言えない。
            ProvisioningState::Unobservable => return None,
            ProvisioningState::Fresh | ProvisioningState::Ready => {}
        }

        if metadata.rebuild.is_some() {
            return Some(NextAction::RebuildPending);
        }
        // まだ何も構築していない案件のDockerfileは、適用済みgenerationとの差分ではない。
        if observation.state == ProvisioningState::Ready
            && observation.current_generation != observation.stored_generation
        {
            return Some(NextAction::RebuildChanged);
        }
        None
    }

    /// 実行するcommand。案件IDを打ち直させない。
    pub fn command(self, project: &str) -> String {
        match self {
            NextAction::RepairPending | NextAction::RepairIncomplete => {
                format!("sbxm repair {project}")
            }
            NextAction::RebuildPending | NextAction::RebuildChanged => {
                format!("sbxm rebuild {project}")
            }
        }
    }

    /// なぜその1手なのかを述べるmessage ID。
    pub fn reason_id(self) -> &'static str {
        match self {
            NextAction::RepairPending => "guidance-next-repair-pending",
            NextAction::RepairIncomplete => "guidance-next-repair-incomplete",
            NextAction::RebuildPending => "guidance-next-rebuild-pending",
            NextAction::RebuildChanged => "guidance-next-rebuild-changed",
        }
    }

    /// この1手が要る案件を、成功として終えてよいか。
    ///
    /// Dockerfileの変更は破損ではなく次の世代交代の入力にすぎないため、成功のまま案内
    /// する。中断した初回構築と未完了の世代交代は、案件がまだ目標構成に達していない
    /// ため失敗として終える。
    pub fn is_blocking(self) -> bool {
        !matches!(self, NextAction::RebuildChanged)
    }

    /// この1手のあとに、世代交代がまだ残り得るか。
    ///
    /// 復旧と世代交代が同時に要る場合も、今すぐ実行できるcommandは1つしか示さない。
    /// 続きがあることだけを持ち、順序を説明できるようにする。
    pub fn leaves_generation_behind(self) -> bool {
        matches!(
            self,
            NextAction::RepairPending | NextAction::RepairIncomplete
        )
    }
}

#[cfg(test)]
#[path = "next_action_test.rs"]
mod next_action_test;
