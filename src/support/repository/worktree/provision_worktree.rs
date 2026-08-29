use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::CreationMode;
use crate::msg;

use crate::support::sandbox;

use crate::support::repository::unusable;

use super::{create_worktree, verify_mode};

/// この実行で用意するworktreeを、起点commitの上に立たせる。
///
/// 中断した作成が残した成果物は作り直さず引き継ぐ。作ったばかりのworktreeは起点commit
/// にいるはずであり、そこにいないものはこの案件の成果物ではない。
pub fn provision_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
    expected_commit: &str,
) -> Result<()> {
    if !sandbox::path_exists(host, sandbox, path)? {
        create_worktree(host, sandbox, git_dir, path, branch, mode)?;
    }
    let head = sandbox::read(host, sandbox, &["git", "-C", path, "rev-parse", "HEAD"])?;
    if head != expected_commit {
        return Err(unusable(
            path,
            msg!(
                "cause-head-differs",
                observed = head,
                expected = expected_commit
            ),
        ));
    }
    verify_mode(host, sandbox, path, branch, mode)
}
