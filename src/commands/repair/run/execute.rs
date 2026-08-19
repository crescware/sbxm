use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::support::provisioning::{self, ProvisioningOutput};

use super::RepairPlan;

/// 診断済みの計画を、共有provisioning境界で実行する。
pub fn execute(
    plan: RepairPlan,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ProvisioningOutput> {
    let RepairPlan {
        mut locked,
        session_lease,
        target_generation,
        warnings,
        preconditions,
        ..
    } = plan;
    let _session_lease = session_lease;
    provisioning::provision(
        &mut locked,
        config,
        &target_generation,
        preconditions,
        host,
        workspace_root,
        progress,
        warnings,
    )
}
