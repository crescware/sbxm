use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

/// branch名の上限。
const MAX_BRANCH_BYTES: usize = 255;

/// remote branch名として受け付けるかを判定する。
///
/// Sandbox内では`git check-ref-format --branch`が同じ値を再検証する。ここでは、
/// 外部commandへ渡す前に確実に拒否できる条件だけを見る。
pub fn validate_branch_name(value: &str) -> Result<()> {
    let invalid = |detail: &'static str| {
        fail(
            ErrorId::InvalidBranchName,
            msg!("error-invalid-branch-name", value = value, detail = detail),
        )
    };

    if value.is_empty() {
        return invalid("the branch name is empty");
    }
    if value.len() > MAX_BRANCH_BYTES {
        return invalid("the branch name is longer than 255 bytes");
    }
    if value.contains('\0') {
        return invalid("the branch name contains a NUL byte");
    }
    if value.contains('\n') || value.contains('\r') {
        return invalid("the branch name contains a line break");
    }
    if value.starts_with('-') {
        // 先頭が`-`の値は外部commandのoptionとして解釈され得る。
        return invalid("the branch name starts with a hyphen");
    }
    Ok(())
}
