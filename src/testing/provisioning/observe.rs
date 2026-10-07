use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::SilentProgress;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::support::provisioning::{self, Observation};

/// 観測結果だけを検査するtestの共通入口。
pub fn observe(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    config: &GlobalConfig,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Observation> {
    provisioning::observe(
        host,
        paths,
        config,
        metadata,
        workspace_root,
        &mut SilentProgress,
    )
}
