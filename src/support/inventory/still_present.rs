use crate::diagnostics::{Error, ErrorId};
use crate::msg;
use crate::project::SandboxName;

/// 削除したはずのSandboxが一覧に残っている。
pub fn still_present(name: &SandboxName) -> Error {
    Error::new(
        ErrorId::SandboxStillPresent,
        msg!("error-sandbox-still-present", sandbox = name),
    )
}
