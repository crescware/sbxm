use crate::command::HostEnvironment;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use crate::support::sandbox;

/// 起点branch名を、Sandbox内の`git check-ref-format --branch`で再検証する。
///
/// repositoryを指定せずに実行するため、`@{-1}`のような文脈依存の短縮形は展開されず、
/// branch名としてそのまま判定される。
pub fn require_branch_name(host: &dyn HostEnvironment, sandbox: &str, branch: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "check-ref-format", "--branch", branch],
    )?;
    if outcome.success() {
        return Ok(());
    }
    fail(
        ErrorId::InvalidBranchName,
        msg!(
            "error-invalid-branch-name",
            value = branch,
            detail = "git in the sandbox does not accept this as a branch name"
        ),
    )
}
