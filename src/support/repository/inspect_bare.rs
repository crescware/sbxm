use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::support::sandbox;

/// bare repositoryの観測と、command開始前の共通工程表示。
/// 不在・破損・観測不能の扱いは、診断または構築側の呼び出し元が決める。
pub fn inspect_bare(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    progress: &mut dyn ProgressSink,
) -> Result<CommandOutcome> {
    progress.step(msg!("progress-inspect-repository"));
    sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    )
}
