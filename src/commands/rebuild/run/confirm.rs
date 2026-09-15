use crate::diagnostics::Result;
use crate::msg;

use crate::support::protection::{self, ConfirmPrompt, ProtectionConfirmation, ProtectionSnapshot};

/// 再構築して良いことを利用者に確かめる。
///
/// rebuildに`--force`は無く、常に対話端末で対象の登録IDの入力を得た場合だけ
/// `ProtectionConfirmation`を返す。非対話環境、cancel、打ち直しの尽きた不一致では
/// 確認を作らずに拒否する。
pub fn confirm(
    snapshot: ProtectionSnapshot,
    project: &str,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
) -> Result<ProtectionConfirmation> {
    protection::confirmation::confirm_interactively(
        snapshot,
        interactive,
        prompt,
        &msg!("rebuild-confirm-prompt", project = project),
    )
}
