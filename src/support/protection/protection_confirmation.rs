use crate::project::SandboxName;

use super::{DestructiveOperation, ProtectionFingerprint};

/// sandbox名の完全一致入力を得た`ProtectionSnapshot`だけから生成できる、opaqueな
/// 確認証跡。
///
/// 別run・別sandbox・別状態で使い回せないよう`Clone`/`Copy`にしない。
#[derive(Debug)]
pub struct ProtectionConfirmation {
    pub(super) operation: DestructiveOperation,
    pub(super) sandbox: SandboxName,
    pub(super) fingerprint: ProtectionFingerprint,
}
