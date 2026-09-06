use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;

use super::{ProvisioningState, require_repair};

/// 中断した初回構築を、成果物を観測せずに拒否する。
///
/// intentはmetadataへ保存されていること自体が中断の証跡であり、Sandboxの有無にも
/// 停止中かどうかにも依らず同じ結論になる。hostへ1 callも出さずに判定できるため、
/// 暗黙に再開しないcommandは、hostへ触れる前にここを通す。`Observation::classify`が
/// `Pending`と呼ぶ事実と同じものを指す。
pub(crate) fn require_no_initial_intent(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.initial_provisioning.is_some() {
        return Err(require_repair(metadata, ProvisioningState::Pending));
    }
    Ok(())
}
