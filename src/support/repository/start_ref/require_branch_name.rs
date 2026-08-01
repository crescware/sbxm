use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
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
    Err(Error::single(
        Diagnostic::new(
            ErrorId::InvalidBranchName,
            msg!("error-invalid-branch-name"),
        )
        .fact(Fact::value(branch))
        .fact(Fact::reason(msg!("cause-refused-by-git"))),
    ))
}
