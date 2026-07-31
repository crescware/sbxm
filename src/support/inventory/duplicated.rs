use crate::diagnostics::{Error, ErrorId};
use crate::msg;

/// 一覧に同じ名前のSandboxが複数あることを、そのまま伝える。
pub(super) fn duplicated(names: &[&str]) -> Error {
    Error::new(
        ErrorId::SandboxNameCollision,
        msg!("error-sandbox-name-duplicated", sandbox = names.join(", ")),
    )
}
