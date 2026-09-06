use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;

use super::{Observation, ProvisioningState, require_observable, require_repair};

/// 初回構築の入口が、観測結果から進める道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitialRoute {
    /// 再利用できる成果物がなく、固定済みintentのもとで構築する。
    Build,
    /// 目標構成が揃っている。何も変更せずそのまま使える。
    AlreadyBuilt,
}

impl InitialRoute {
    /// 初回構築を行う入口が、観測結果から道を決める唯一の規則。
    ///
    /// `prepare`と`open`が同じ事実へ別の規則を当てないよう、拒否する理由もここが決める。
    /// 中断した初回構築と、欠落を観測した成果物は暗黙に再開せず、明示的なrepairへ渡す。
    pub(crate) fn decide(
        metadata: &ProjectMetadata,
        observation: &Observation,
    ) -> Result<InitialRoute> {
        // 観測は最後まで並べるが、安全と確認できなかった事実が1つでもあれば、hostを
        // 変更する前にここで止める。
        observation.require_safe()?;
        match observation.state {
            ProvisioningState::Fresh => Ok(InitialRoute::Build),
            ProvisioningState::Ready => Ok(InitialRoute::AlreadyBuilt),
            ProvisioningState::Pending | ProvisioningState::Incomplete => {
                Err(require_repair(metadata, observation.state))
            }
            // 停止中のSandboxは、中を読めば起動してしまう。欠落と決めてrepairへ送らず、
            // 完成と決めて構築を飛ばすこともせず、観測できない事実として拒否する。
            ProvisioningState::Unobservable => Err(require_observable(metadata, observation)),
        }
    }
}
