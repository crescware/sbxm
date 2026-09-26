use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::paths::LOCK_TIMEOUT;
use crate::project::ProjectId;
use crate::support::host_sync::AutoSaved;
use crate::support::select;

use super::save_selected;

/// hostにあるrepositoryの案件で、Sandboxが動いていれば、先にcommitをhostへ保存する。
///
/// Sandboxを作り直す、消す前と、sessionを閉じたあとに呼ぶ。案件を決められなければ何も
/// しない。続く操作が同じ理由を自分の規則で報告する。
pub fn save_first(
    location: &ConfigLocation,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> AutoSaved {
    match select::find(location, project) {
        Ok(candidate) => save_selected(candidate, host, workspace_root, LOCK_TIMEOUT, progress),
        Err(_) => AutoSaved::Nothing,
    }
}

#[cfg(test)]
#[path = "save_first_test.rs"]
mod save_first_test;
