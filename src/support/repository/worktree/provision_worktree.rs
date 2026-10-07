use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::CreationMode;
use crate::msg;

use crate::support::sandbox;
use crate::support::worktree::Inspection;

use crate::support::repository::unusable;

use super::{create_worktree, verify_mode};

/// この実行で用意するworktreeを、起点commitの上に立たせる。
///
/// 中断した作成が残した成果物は作り直さず引き継ぐ。作ったばかりのworktreeは起点commit
/// にいるはずであり、そこにいないものはこの案件の成果物ではない。起点commitは、
/// hostから戻したbranchならその先端、それ以外はoriginの先端である。
pub fn provision_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    inspection: &mut Inspection<'_>,
    branch: &str,
    mode: CreationMode,
    expected_commit: &str,
) -> Result<()> {
    if !sandbox::path_exists(host, sandbox, inspection.path())? {
        create_worktree(host, sandbox, git_dir, inspection.path(), branch, mode)?;
    }
    let head = inspection.head()?;
    if head != expected_commit {
        return Err(unusable(
            inspection.path(),
            msg!(
                "cause-head-differs",
                observed = head,
                expected = expected_commit
            ),
        ));
    }
    verify_mode(inspection, branch, mode)
}
