use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::super::{ProtectionConfirmation, ProtectionSnapshot};

/// `entered_sandbox`が`snapshot`の対象sandbox名とbyte単位で完全一致した場合だけ、
/// `snapshot`をconsumeして確認証跡を返す。
///
/// 一致しない場合は`snapshot`ごと破棄する。空文字列や大小文字違いも不一致として扱う。
pub fn confirm(
    snapshot: ProtectionSnapshot,
    entered_sandbox: &str,
) -> Result<ProtectionConfirmation> {
    let sandbox = snapshot.assessment.sandbox().clone();
    if entered_sandbox.as_bytes() != sandbox.as_str().as_bytes() {
        return Err(confirmation_mismatch(sandbox.as_str()));
    }
    Ok(ProtectionConfirmation {
        operation: snapshot.assessment.operation(),
        sandbox,
        fingerprint: snapshot.fingerprint,
    })
}

/// 入力が一致しない場合のerror。
fn confirmation_mismatch(expected: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProtectionNotConfirmed,
            msg!("error-protection-not-confirmed", sandbox = expected),
        )
        .remediation(msg!("remediation-protection-not-confirmed")),
    )
}
