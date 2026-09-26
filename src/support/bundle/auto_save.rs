use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::project::SandboxLayout;
use crate::support::repository;

use super::{AutoSaved, save_failed, save_to_host};

/// hostにあるrepositoryを登録した案件で、Sandboxのcommitをhostへ保存しておく。
///
/// 失われうるのは最後に保存したあとの作業だけであり、保存する機会を増やしてその範囲を
/// 狭める。Sandboxを止める、作り直す、消す前と、sessionを閉じたあとに呼ぶ。保存できなくても
/// 呼び出し側の操作は止めず、warningとして伝える。GitHubの案件では何もしない。
///
/// Sandboxが動いていることと、project lockを持っていることは呼び出し側が確かめておく。
pub fn auto_save(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    progress: &mut dyn ProgressSink,
) -> AutoSaved {
    if metadata.repository.host_path().is_none() {
        return AutoSaved::Nothing;
    }
    let sandbox = metadata.sandbox_name();
    let target = repository::host_repository(paths, metadata);
    let git_dir = SandboxLayout::new(metadata.canonical_id()).bare_git_dir();
    let project = metadata.display_id();
    // 初めての保存は履歴全体を運ぶ。黙って待たせず、何をしているかを先に示す。
    progress.step(msg!("progress-saving-to-host", project = project.clone()));
    match save_to_host(host, &sandbox, &git_dir, &target) {
        Ok(Some(changes)) if !changes.is_empty() => AutoSaved::Saved(msg!(
            "auto-save-done",
            project = project,
            repository = paths::display(&target),
            namespace = sandbox.as_str()
        )),
        Ok(_) => AutoSaved::Nothing,
        Err(error) => AutoSaved::Failed(save_failed(&project, &error)),
    }
}
