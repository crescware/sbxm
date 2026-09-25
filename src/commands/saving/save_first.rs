use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::ConfigLocation;
use crate::project::ProjectId;
use crate::support::bundle::{self, AutoSaved};
use crate::support::{inventory, select};

/// hostにあるrepositoryの案件で、Sandboxが動いていれば、先にcommitをhostへ保存する。
///
/// Sandboxを作り直す、消す前と、sessionを閉じたあとに呼ぶ。案件を決められない、
/// Sandboxが動いていないといった場合は何もしない。続く操作が同じ理由を自分の規則で
/// 報告する。lockを取れなければ、保存できなかったことをwarningにする。続く操作が同じ
/// 理由で断るとは限らず、黙れば保存したと受け取られる。保存はproject lockを持って行い、
/// 終われば手放す。
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
    if candidate.repository.host_path().is_none() {
        return AutoSaved::Nothing;
    }
    let display_id = candidate.display_id();
    let locked = match candidate.lock() {
        Ok(locked) => locked,
        Err(error) => return AutoSaved::Failed(bundle::save_failed(&display_id, &error)),
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
