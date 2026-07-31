use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxLayout;

use crate::support::sandbox;

use super::{Entry, parse_list};

/// Sandbox内のworktree一覧を読む。
///
/// 一覧を読めない場合は、worktreeが無いことと区別して外部commandの失敗とする。
pub fn list(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
) -> Result<Vec<Entry>> {
    let git_dir = layout.bare_git_dir();
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ],
    )?;
    let outcome = outcome.require_success()?;
    parse_list(&outcome.stdout_text())
}
