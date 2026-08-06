use crate::diagnostics::Result;
use crate::msg;

use crate::support::protection::{self, ConfirmPrompt, ProtectionConfirmation, ProtectionSnapshot};

/// 再構築して良いことを利用者に確かめる。
///
/// rebuildに`--force`は無く、常に対話端末でSandbox名の完全一致入力を得た場合だけ
/// `ProtectionConfirmation`を返す。非対話環境、cancel、名前不一致では確認を作らずに
/// 拒否する。
pub fn confirm(
    snapshot: ProtectionSnapshot,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
) -> Result<ProtectionConfirmation> {
    protection::confirmation::confirm_interactively(
        snapshot,
        interactive,
        prompt,
        &msg!("rebuild-confirm-prompt"),
    )
}
