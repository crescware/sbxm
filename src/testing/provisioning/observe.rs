use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::SilentProgress;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::support::daemon;
use crate::support::provisioning::{self, Observation};

/// 観測結果だけを検査するtestの共通入口。本番の呼び出し元と同じく、一覧を読んでから渡す。
pub fn observe(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Observation> {
    let entries = daemon::list(host)?;
    provisioning::observe(
        host,
        &entries,
        paths,
        metadata,
        workspace_root,
        &mut SilentProgress,
    )
}
