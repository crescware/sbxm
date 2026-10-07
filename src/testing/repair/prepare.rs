use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::commands::repair::run::Prepared;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::SilentProgress;
use crate::diagnostics::Result;
use crate::project::ProjectId;
use crate::support::select::ProjectPrompt;

/// repairの判定・planだけを検査するtestの共通入口。
pub fn prepare(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Prepared> {
    crate::commands::repair::run::prepare(
        location,
        config,
        requested,
        host,
        workspace_root,
        prompt,
        &mut SilentProgress,
    )
}
