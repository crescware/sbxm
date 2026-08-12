use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::super::{ProtectionConfirmation, ProtectionPermit, ProtectionSnapshot};
use super::require_no_blockers;

/// remove直前に取り直した`current`が、`confirmation`が確認した状態とまだ一致しており、
/// かつ拒否理由（`Blocker`）が1件も無い場合だけ許可証を発行する。
///
/// 拒否理由が無くても、`confirmation`のsandbox名・操作種別・fingerprintのいずれかが
/// `current`と食い違えば、確認した瞬間から状態が変わったとみなし、内容を示さずに拒否する
/// （食い違いの内容を表示すると、確認済みの状態を引数として渡すだけで通せる情報になる）。
///
/// `confirmation`と`current`をconsumeするのは意図的である。古い確認証跡や観測結果を
/// 使い回して再度許可を得ることを型で防ぐ。
#[allow(clippy::needless_pass_by_value)]
pub fn authorize(
    confirmation: ProtectionConfirmation,
    current: ProtectionSnapshot,
) -> Result<ProtectionPermit> {
    require_no_blockers(current.assessment())?;
    let matches = confirmation.operation == current.assessment().operation()
        && &confirmation.sandbox == current.assessment().sandbox()
        && &confirmation.fingerprint == current.fingerprint();
    if !matches {
        return Err(state_changed_since_confirmation(
            confirmation.sandbox.as_str(),
        ));
    }
    Ok(ProtectionPermit::issue())
}

/// 確認した瞬間から現在までのあいだに、対象・種別・観測結果のいずれかが変わった。
fn state_changed_since_confirmation(sandbox: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProtectionStateChanged,
            msg!("error-protection-state-changed", sandbox = sandbox),
        )
        .remediation(msg!("remediation-protection-state-changed")),
    )
}
