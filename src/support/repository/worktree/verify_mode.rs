use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::CreationMode;

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
                    &format!("the worktree is not on {expected}"),
                ));
            }
        }
        CreationMode::Detached => {
            if outcome.success() {
                return Err(unusable(
                    path,
                    &format!("the worktree is on {observed}, and this project uses detached heads"),
                ));
            }
        }
    }
    Ok(())
}
