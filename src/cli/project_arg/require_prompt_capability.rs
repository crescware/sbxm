use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use crate::cli::Interactivity;

/// 非TTYで対象を省略した場合は、外部状態を読む前に終了する。
pub fn require_prompt_capability(interactivity: Interactivity, command: &str) -> Result<()> {
    if interactivity.can_prompt() {
        return Ok(());
    }
    fail(
        ErrorId::ProjectArgumentRequired,
        msg!("error-project-argument-required", subcommand = command),
    )
}
