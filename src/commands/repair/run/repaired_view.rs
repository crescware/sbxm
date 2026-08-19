use crate::support::provisioning::ProvisioningOutput;

use super::{Phase, View};

pub fn repaired_view(output: &ProvisioningOutput, target: &str) -> View {
    View {
        project: output.project.clone(),
        sandbox: Some(output.sandbox.clone()),
        target_generation: Some(target.to_string()),
        artifacts: Vec::new(),
        phase: Phase::Repaired,
    }
}
