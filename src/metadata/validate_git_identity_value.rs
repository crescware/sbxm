use crate::diagnostics::Msg;
use crate::msg;

/// Git identityの値として使えるか。
///
/// 受け付けられない理由は、報告する側が文へ連結できる原文ではなくmessageで返す。
/// 英語の断片を返すと、それを受け取った診断が翻訳文のなかへ英語を混ぜてしまう。
pub fn validate_git_identity_value(value: &str) -> std::result::Result<(), Msg> {
    if value.trim().is_empty() {
        return Err(msg!("cause-value-empty"));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(msg!("cause-value-has-line-break"));
    }
    Ok(())
}
