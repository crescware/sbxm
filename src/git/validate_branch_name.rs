use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

/// branch名の上限。
const MAX_BRANCH_BYTES: usize = 255;

/// remote branch名として受け付けるかを判定する。
///
/// Sandbox内では`git check-ref-format --branch`が同じ値を再検証する。ここでは、
/// 外部commandへ渡す前に確実に拒否できる条件だけを見る。
pub fn validate_branch_name(value: &str) -> Result<()> {
    let invalid = |reason: Msg| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::InvalidBranchName,
                msg!("error-invalid-branch-name"),
            )
            .fact(Fact::value(value))
            .fact(Fact::reason(reason)),
        ))
    };

    if value.is_empty() {
        return invalid(msg!("cause-value-empty"));
    }
    if value.len() > MAX_BRANCH_BYTES {
        return invalid(msg!("cause-longer-than", maximum = MAX_BRANCH_BYTES));
    }
    if value.contains('\0') {
        return invalid(msg!("cause-contains-nul"));
    }
    if value.contains('\n') || value.contains('\r') {
        return invalid(msg!("cause-value-has-line-break"));
    }
    if value.starts_with('-') {
        // 先頭が`-`の値は外部commandのoptionとして解釈され得る。
        return invalid(msg!("cause-starts-with-hyphen"));
    }
    Ok(())
}
