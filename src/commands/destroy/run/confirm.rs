use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{ConfirmPrompt, Prepared};

/// 削除して良いことを利用者に確かめる。
///
/// force modeと非対話では対話確認を行わない。TTYの通常modeだけがSandbox名の完全
/// 入力を求め、一致しなければ何も削除しない。
pub fn confirm(
    prepared: &Prepared,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
) -> Result<()> {
    if prepared.force || !interactive {
        return Ok(());
    }
    if prompt.confirm_sandbox_name(&prepared.plan.sandbox)? {
        return Ok(());
    }
    Err(confirmation_mismatch(&prepared.plan.sandbox))
}

/// 入力が一致しない場合のerror。
fn confirmation_mismatch(expected: &str) -> Error {
    Error::new(
        ErrorId::DestroyNotConfirmed,
        msg!("error-destroy-not-confirmed", sandbox = expected),
    )
}
