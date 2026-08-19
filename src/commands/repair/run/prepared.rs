use crate::support::provisioning::ProvisioningOutput;

use super::{Phase, RepairPlan, View};

/// repairのread-only診断結果と、変更前に表示する計画。
pub enum Prepared {
    Fresh { project: String },
    Healthy { output: ProvisioningOutput },
    Repairable(Box<RepairPlan>),
}

impl Prepared {
    pub fn view(&self) -> View {
        match self {
            Self::Fresh { project } => View {
                project: project.clone(),
                sandbox: None,
                target_generation: None,
                artifacts: Vec::new(),
                phase: Phase::Fresh,
            },
            Self::Healthy { output } => View {
                project: output.project.clone(),
                sandbox: Some(output.sandbox.clone()),
                target_generation: None,
                artifacts: Vec::new(),
                phase: Phase::Healthy,
            },
            Self::Repairable(plan) => View {
                project: plan.project.clone(),
                sandbox: Some(plan.sandbox.clone()),
                target_generation: Some(plan.target_generation.clone()),
                artifacts: plan.artifacts.clone(),
                phase: Phase::Plan,
            },
        }
    }
}
