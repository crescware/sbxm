use crate::metadata::ProjectMetadata;
use crate::project::{SandboxLayout, SandboxName};

use super::DestructiveOperation;

/// 保護ゲートへ渡す入力。
///
/// 個々のfieldはgate配下のcollectorだけが読む。呼び出し側は`new`だけを使う。
pub struct Request<'a> {
    pub(super) operation: DestructiveOperation,
    pub(super) sandbox: &'a SandboxName,
    pub(super) layout: &'a SandboxLayout,
    pub(super) metadata: &'a ProjectMetadata,
}

impl<'a> Request<'a> {
    pub fn new(
        operation: DestructiveOperation,
        sandbox: &'a SandboxName,
        layout: &'a SandboxLayout,
        metadata: &'a ProjectMetadata,
    ) -> Request<'a> {
        Request {
            operation,
            sandbox,
            layout,
            metadata,
        }
    }
}
