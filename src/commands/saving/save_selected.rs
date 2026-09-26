use std::path::Path;
use std::time::Duration;

use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::support::bundle::{self, AutoSaved};
use crate::support::inventory;
use crate::support::select::Candidate;

/// 選んだ案件がhostにあるrepositoryの案件で、Sandboxが動いていれば、commitをhostへ
/// 保存する。
///
/// Sandboxが動いていなければ何もしない。続く操作が自分の規則で報告する。lockを`wait`
/// のうちに取れなければ、保存できなかったことをwarningにする。続く操作が同じ理由で
/// 断るとは限らず、黙れば保存したと受け取られる。保存はその案件のproject lockだけを
/// 持って行い、終われば手放す。
pub fn save_selected(
    candidate: Candidate,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    wait: Duration,
    progress: &mut dyn ProgressSink,
) -> AutoSaved {
    // GitHubの案件にはlockも取らない。
    if candidate.repository.host_path().is_none() {
        return AutoSaved::Nothing;
    }
    let display_id = candidate.display_id();
    let locked = match candidate.lock_within(wait) {
        Ok(locked) => locked,
        Err(error) => return AutoSaved::Failed(bundle::save_failed(&display_id, &error)),
    };
    // 世代の切替の途中は保存しない。続きの`rebuild`へ任せる。
    if locked.metadata.rebuild.is_some()
        || inventory::require_running(host, &locked.metadata, workspace_root).is_err()
    {
        return AutoSaved::Nothing;
    }
    bundle::auto_save(host, &locked.paths, &locked.metadata, progress)
}
