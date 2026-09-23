use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::paths;

use super::SandboxHomeRelativePath;

/// 配置先が`destination`である宣言fileが無いことを報告する。
pub fn file_not_declared(destination: &SandboxHomeRelativePath) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::FileNotDeclared,
            msg!(
                "error-file-not-declared",
                destination = paths::display(destination.as_path())
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-file-not-declared")).try_run("sbxm files ls"),
        ),
    )
}
