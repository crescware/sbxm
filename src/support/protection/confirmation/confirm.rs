use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::super::{ProtectionConfirmation, ProtectionSnapshot};
use super::matches;

/// `entered`が対象を名指した場合だけ、`snapshot`をconsumeして確認証跡を返す。
///
/// 名指しと認めるのは、案件の登録ID（`<owner>/<repository>`）と、そこから導いたsandbox名の
/// 2つである。訊くのは登録IDの側とする。sandbox名は末尾にsbxmが内部で使うhashを持ち、
/// 利用者が覚えている値ではないためである。画面が示すsandbox名を打ち返した場合も、同じ
/// 対象を名指したものとして受け取る。
///
/// 一致しない場合は`snapshot`ごと破棄する。
pub fn confirm(snapshot: ProtectionSnapshot, entered: &str) -> Result<ProtectionConfirmation> {
    if !matches(&snapshot, entered) {
        return Err(confirmation_mismatch(snapshot.assessment.project()));
    }
    Ok(ProtectionConfirmation {
        operation: snapshot.assessment.operation(),
        sandbox: snapshot.assessment.sandbox().clone(),
        fingerprint: snapshot.fingerprint,
    })
}

/// 入力が一致しない場合のerror。
fn confirmation_mismatch(expected: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProtectionNotConfirmed,
            msg!("error-protection-not-confirmed", project = expected),
        )
        .remediation(msg!("remediation-protection-not-confirmed")),
    )
}
