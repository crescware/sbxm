use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::CreationMode;
use crate::msg;

use crate::support::sandbox;

use crate::support::repository::unusable;

/// worktreeが宣言どおりのmodeであることを確認する。
pub fn verify_mode(
    host: &dyn HostEnvironment,
    sandbox: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "-C", path, "symbolic-ref", "-q", "HEAD"],
    )?;
    let observed = outcome.stdout_text().trim().to_string();
    match mode {
        CreationMode::Attached => {
            let expected = format!("refs/heads/{branch}");
            if !outcome.success() || observed != expected {
                return Err(unusable(
                    path,
                    msg!("cause-worktree-not-on-branch", expected = expected),
                ));
            }
        }
        CreationMode::Detached => {
            if outcome.success() {
                return Err(unusable(
                    path,
                    msg!("cause-worktree-on-a-branch", observed = observed),
                ));
            }
        }
    }
    Ok(())
}
