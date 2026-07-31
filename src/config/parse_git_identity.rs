use std::path::Path;

use crate::diagnostics::Result;
use crate::metadata::{GitIdentity, validate_git_identity_value};

use super::{invalid_value, missing_field};

/// 保存済みの名義を読む。
///
/// 名義は2つで1つの意図である。片方だけの宣言から残りを推測して補わない。
pub(super) fn parse_git_identity(
    user_name: Option<String>,
    user_email: Option<String>,
    path: &Path,
) -> Result<Option<GitIdentity>> {
    match (user_name, user_email) {
        (None, None) => Ok(None),
        (Some(user_name), Some(user_email)) => {
            validate_git_identity_value(&user_name)
                .map_err(|detail| invalid_value(path, "git_user_name", detail))?;
            validate_git_identity_value(&user_email)
                .map_err(|detail| invalid_value(path, "git_user_email", detail))?;
            Ok(Some(GitIdentity {
                user_name,
                user_email,
            }))
        }
        (Some(_), None) => Err(missing_field(path, "git_user_email")),
        (None, Some(_)) => Err(missing_field(path, "git_user_name")),
    }
}
