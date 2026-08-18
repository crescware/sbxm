use super::{Artifact, ProvisioningOutput, ProvisioningState};

/// read-only観測から作った初回構築状態。
#[derive(Debug, Clone)]
pub struct Observation {
    pub state: ProvisioningState,
    pub output: Option<ProvisioningOutput>,
    pub artifacts: Vec<Artifact>,
}
