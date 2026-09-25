use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::support::bundle::{self, AutoSaved};
use crate::support::inventory;
use crate::support::select::Candidate;

/// 選んだ案件がhostにあるrepositoryの案件で、Sandboxが動いていれば、commitをhostへ
/// 保存する。
///
/// Sandboxが動いていなければ何もしない。続く操作が自分の規則で報告する。lockを取れ
/// なければ、保存できなかったことをwarningにする。続く操作が同じ理由で断るとは限らず、
/// 黙れば保存したと受け取られる。保存はその案件のproject lockだけを持って行い、終われば
/// 手放す。
pub fn save_selected(
    candidate: Candidate,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> AutoSaved {
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
    bundle::auto_save(host, &locked.paths, &locked.metadata, progress)
}
