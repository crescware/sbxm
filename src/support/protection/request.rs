use std::path::Path;

use crate::metadata::ProjectMetadata;
use crate::project::{SandboxLayout, SandboxName};

use super::DestructiveOperation;

/// 保護ゲートへ渡す入力。
///
/// 個々のfieldはgate配下のcollectorだけが読む。呼び出し側は`new`だけを使う。
pub struct Request<'a> {
    pub(super) operation: DestructiveOperation,
    pub(super) sandbox: &'a SandboxName,
    /// 中立workspace directoryを置くhost側のroot。
    ///
    /// Sandboxの中を見るcommandへ頼る前に、mount元がhostに在ることを直接確かめる。
    pub(super) workspace_root: &'a Path,
    pub(super) layout: &'a SandboxLayout,
    pub(super) metadata: &'a ProjectMetadata,
}

impl<'a> Request<'a> {
    pub fn new(
        operation: DestructiveOperation,
        sandbox: &'a SandboxName,
        workspace_root: &'a Path,
        layout: &'a SandboxLayout,
        metadata: &'a ProjectMetadata,
    ) -> Request<'a> {
        Request {
            operation,
            sandbox,
            workspace_root,
            layout,
            metadata,
        }
    }
}
