use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Warning};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::project::SandboxLayout;
use crate::support::repository;

use super::{AutoSaved, save_to_host};

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
) -> AutoSaved {
    if metadata.repository.host_path().is_none() {
        return AutoSaved::Nothing;
    }
    let sandbox = metadata.sandbox_name();
    let target = repository::host_repository(paths, metadata);
    let git_dir = SandboxLayout::new(metadata.canonical_id()).bare_git_dir();
    let project = metadata.display_id();
    match save_to_host(host, paths, &sandbox, &git_dir, &target) {
        Ok(Some(changes)) if !changes.is_empty() => AutoSaved::Saved(msg!(
            "auto-save-done",
            project = project,
            repository = paths::display(&target),
            namespace = sandbox.as_str()
        )),
        Ok(_) => AutoSaved::Nothing,
        Err(error) => {
            let mut warning = Warning::text(msg!("auto-save-failed", project = project.clone()));
            if let Some(diagnostic) = error.diagnostics().first() {
                warning = warning.fact(Fact::cause(diagnostic.id.as_str()));
            }
            AutoSaved::Failed(warning.try_run(format!("sbxm fetch {project}")))
        }
    }
}
