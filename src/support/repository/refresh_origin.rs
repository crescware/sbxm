use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::ProgressSink;
use crate::diagnostics::Result;

use crate::support::sandbox;

/// bare repositoryのoriginを`fetch --prune`で最新化する。
///
/// 進捗を見せる呼び出しは`progress`へ`Some`を渡して`--progress`付きで中継し、
/// 見せない呼び出しは`None`を渡して静かに実行する。fetchの引数はこの関数だけが持ち、
/// 呼び出し側ごとに別実装を増やさない。
pub fn refresh_origin(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    progress: Option<&mut dyn ProgressSink>,
) -> Result<CommandOutcome> {
    match progress {
        Some(progress) => sandbox::exec_with_progress(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                git_dir,
                "fetch",
                "--prune",
                "--progress",
                "origin",
            ],
            progress,
        ),
        None => sandbox::exec(
            host,
            sandbox,
            &["git", "--git-dir", git_dir, "fetch", "--prune", "origin"],
        ),
    }
}
