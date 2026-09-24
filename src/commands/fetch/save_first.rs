use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::project::ProjectId;
use crate::repository::Provider;
use crate::support::bundle::{self, AutoSaved};
use crate::support::{inventory, select};

/// hostにあるrepositoryの案件で、Sandboxが動いていれば、先にcommitをhostへ保存する。
///
/// Sandboxを作り直す、消す前と、sessionを閉じたあとに呼ぶ。案件を決められない、lockを
/// 取れない、Sandboxが動いていないといった場合は何もしない。続く操作が同じ理由を
/// 自分の規則で報告する。保存はproject lockを持って行い、終われば手放す。
pub fn save_first(
    location: &ConfigLocation,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> AutoSaved {
    let Ok(candidate) = select::find(location, project) else {
        return AutoSaved::Nothing;
    };
    // GitHubの案件にはlockも取らない。
    if candidate.repository.provider() != Provider::Local {
        return AutoSaved::Nothing;
    }
    let Ok(locked) = candidate.lock() else {
        return AutoSaved::Nothing;
    };
    // 世代の切替の途中は`sbxm fetch`も断る。続きの`rebuild`へ任せる。
    if locked.metadata.rebuild.is_some()
        || inventory::require_running(host, &locked.metadata, workspace_root).is_err()
    {
        return AutoSaved::Nothing;
    }
    bundle::auto_save(host, &locked.paths, &locked.metadata)
}

#[cfg(test)]
#[path = "save_first_test.rs"]
mod save_first_test;
