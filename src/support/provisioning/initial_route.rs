use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;

use super::{Observation, ProvisioningState, require_observable, require_open, select_generation};

/// 初回構築の入口が、観測結果から進める道。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InitialRoute {
    /// 再利用できる成果物がなく、固定済みintentのもとで構築する。
    ///
    /// `target`は構築が向かうgeneration。`None`は現在のDockerfileをそのまま採用する
    /// 最初の構築、`Some`は`select_generation`が選んだ、旧世代の成果物を保持したまま
    /// 続ける対象である。
    Build { target: Option<String> },
    /// 目標構成が揃っている。何も変更せずそのまま使える。
    AlreadyBuilt,
    /// 固定済みintentと観測した成果物から、不足工程を続ける。
    Resume,
}

impl InitialRoute {
    /// 初回構築を行う入口が、観測結果から道を決める唯一の規則。
    ///
    /// `prepare`と`open`が同じ事実へ別の規則を当てないよう、拒否する理由もここが決める。
    /// 中断した初回構築は固定済み入力から再開し、根拠のない欠落だけをopenへ渡す。
    pub(crate) fn decide(
        metadata: &ProjectMetadata,
        observation: &Observation,
    ) -> Result<InitialRoute> {
        // 観測は最後まで並べるが、安全と確認できなかった事実が1つでもあれば、hostを
        // 変更する前にここで止める。
        observation.require_safe()?;
        match observation.state {
            ProvisioningState::Fresh => Ok(InitialRoute::Build { target: None }),
            ProvisioningState::Ready => Ok(InitialRoute::AlreadyBuilt),
            ProvisioningState::Pending => Ok(InitialRoute::Resume),
            ProvisioningState::Incomplete if observation.sandbox.is_missing() => {
                // intentが無いこの経路は`repair`と同じ規則で対象を選ぶ。旧世代の
                // image/templateが実測できれば、それを保持したまま完成させる。
                Ok(InitialRoute::Build {
                    target: Some(select_generation(observation, metadata, false)?),
                })
            }
            ProvisioningState::Incomplete => Err(require_open(metadata, observation.state)),
            // 停止中のSandboxは、中を読めば起動してしまう。欠落と決めてopenへ送らず、
            // 完成と決めて構築を飛ばすこともせず、観測できない事実として拒否する。
            ProvisioningState::Unobservable => Err(require_observable(metadata, observation)),
        }
    }
}
