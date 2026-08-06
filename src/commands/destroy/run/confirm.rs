use crate::diagnostics::Result;
use crate::msg;

use crate::support::protection::{self, ConfirmPrompt, ProtectionConfirmation};

use super::Prepared;

/// 削除して良いことを利用者に確かめる。
///
/// force mode、またはSandboxが元から無い場合は確認を求めない。それ以外は対話端末で
/// Sandbox名の完全一致入力を得た場合だけ`ProtectionConfirmation`を返す。非対話環境、
/// cancel、名前不一致では確認を作らずに拒否する。
pub fn confirm(
    prepared: &mut Prepared,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
) -> Result<Option<ProtectionConfirmation>> {
    let Some(snapshot) = prepared.snapshot.take() else {
        return Ok(None);
    };
    let confirmation = protection::confirmation::confirm_interactively(
        snapshot,
        interactive,
        prompt,
        &msg!("destroy-confirm-prompt"),
    )?;
    Ok(Some(confirmation))
}
