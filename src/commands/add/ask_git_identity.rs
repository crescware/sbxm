use crate::command::HostEnvironment;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::metadata::{GitIdentity, validate_git_identity_value};
use crate::msg;
use crate::support::identity;

use super::IdentityPrompt;

/// 名義を選ばせる。
///
/// hostが宣言している値を初期値として置く。読めない場合は空欄で始まり、それ自体は
/// 失敗ではない。
pub fn ask_git_identity(
    prompt: &mut dyn IdentityPrompt,
    host: &dyn HostEnvironment,
) -> Result<GitIdentity> {
    let typed_name = prompt.git_user_name(&identity::candidate_from_host(host, "user.name"))?;
    let typed_email = prompt.git_user_email(&identity::candidate_from_host(host, "user.email"))?;
    Ok(GitIdentity {
        user_name: accept("user.name", &typed_name)?,
        user_email: accept("user.email", &typed_email)?,
    })
}

/// 入力された1行を名義の値として受け取る。
fn accept(field: &str, value: &str) -> Result<String> {
    let value = value.trim();
    match validate_git_identity_value(value) {
        Ok(()) => Ok(value.to_string()),
        Err(detail) => fail(
            ErrorId::InvalidValue,
            msg!("error-git-identity-invalid", field = field, detail = detail),
        ),
    }
}
