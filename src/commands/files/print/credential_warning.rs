use crate::design::Warning;
use crate::msg;
use crate::paths;

use crate::commands::files::Added;

/// 認証情報を持つfileによく使われる名前への注意。宣言は取り消さない。
pub fn credential_warning(added: &Added) -> Warning {
    Warning::text(msg!(
        "warning-file-looks-like-credential",
        source = paths::display(added.declaration.source.as_path())
    ))
    .explain(msg!("files-secret-hint"))
}
