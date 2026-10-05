use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::commands::status::project::ProjectStatus;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::SilentProgress;
use crate::diagnostics::Result;
use crate::project::ProjectId;

/// 診断結果を調べるtestが共有する入口。
///
/// 実行中の表示を調べるtestは、commandまたは本番の診断関数を直接使う。
pub fn diagnose(
    location: &ConfigLocation,
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<ProjectStatus> {
    crate::commands::status::project::diagnose(
        location,
        config,
        project,
        host,
        workspace_root,
        &mut SilentProgress,
    )
}
