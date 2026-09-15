use crate::diagnostics::{Msg, Result};
use crate::msg;

use super::super::{ConfirmPrompt, ProtectionConfirmation, ProtectionSnapshot};
use super::{confirm, matches};

/// 同じ計画に対して入力を受け付ける回数。
///
/// 1回で打ち切ると、打ち間違いのたびに観測からやり直すことになる。停止していたSandboxを
/// 起動した実行では、その起動も含めてやり直しになる。打ち直しは同じ`snapshot`の上で
/// 行うため、確認と削除のあいだに挟まる観測は増えない。
const ATTEMPTS: usize = 3;

/// 削除計画を見せたあと、対話端末での対象名の入力だけを合図に確認証跡を作る。
///
/// 一致しない入力は、`ATTEMPTS`回に達するまで打ち直しを求める。何を打てば続くかは
/// promptが毎回示す。cancel（EscまたはCtrl-C）は`Error::Canceled`のまま伝え、打ち直しの
/// 対象にしない。
///
/// 非対話環境では、答える手段がないため入力を待たずに拒否する（対象を名指さない空文字列と
/// 比べることで、[`confirm`]の同じ不一致経路をそのまま再利用する）。
pub fn confirm_interactively(
    snapshot: ProtectionSnapshot,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
    heading: &Msg,
) -> Result<ProtectionConfirmation> {
    if !interactive {
        return confirm(snapshot, "");
    }
    let mut answered = prompt.read_confirmation(heading)?;
    let mut left = ATTEMPTS - 1;
    while left > 0 && !matches(&snapshot, &answered) {
        let again = msg!(
            "confirm-retry-prompt",
            project = snapshot.assessment.project(),
            attempts = left
        );
        answered = prompt.read_confirmation(&again)?;
        left -= 1;
    }
    confirm(snapshot, &answered)
}
