use crate::diagnostics::{Msg, Result};

use super::super::{ConfirmPrompt, ProtectionConfirmation, ProtectionSnapshot};
use super::confirm;

/// 削除計画を見せたあと、対話端末でのSandbox名の完全一致入力だけを合図に確認証跡を作る。
///
/// 非対話環境では、答える手段がないため入力を待たずに拒否する（`sbx`が持たない
/// sandbox名と比べることで、[`confirm`]の同じ不一致経路をそのまま再利用する）。
/// cancel（EscまたはCtrl-C）は`Error::Canceled`のまま伝える。
pub fn confirm_interactively(
    snapshot: ProtectionSnapshot,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
    heading: &Msg,
) -> Result<ProtectionConfirmation> {
    if !interactive {
        return confirm(snapshot, "");
    }
    let typed = prompt.read_sandbox_name(heading)?;
    confirm(snapshot, &typed)
}
