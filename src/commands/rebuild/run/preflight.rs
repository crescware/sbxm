use super::Prepared;
use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::support::{
    inventory,
    provisioning::{self, SnapshotFile},
    template::LoadedTemplate,
};

/// 時間のかかる容量確認は、最後の保護観測よりも先に終える。
#[allow(clippy::too_many_arguments)]
pub(super) fn preflight(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    template: &LoadedTemplate,
    config: &GlobalConfig,
    workspace_root: &std::path::Path,
    existed: bool,
    poll: inventory::Poll,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<SnapshotFile>> {
    if !config.files.is_empty() {
        progress.step(msg!("progress-status-files"));
    }
    let inputs = provisioning::ProvisioningInputs::capture_files(&prepared.locked.paths, config)?;
    super::check_capacity::check_capacity(
        host,
        &prepared.locked.paths,
        workspace_root,
        &prepared.locked.metadata,
        template,
        existed,
        &inputs,
        poll,
        progress,
    )?;
    Ok(inputs)
}
