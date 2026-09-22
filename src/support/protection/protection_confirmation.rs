use crate::project::SandboxName;

use super::{DestructiveOperation, ProtectionFingerprint};

/// 対象を名指す入力を得た`ProtectionSnapshot`だけから生成できる、opaqueな確認証跡。
///
/// 利用者へ訊くのは案件の登録IDだが、証跡が持つのはsandbox名である。`authorize`が
/// remove直前の観測と突き合わせる相手は、利用者の打鍵ではなく観測した対象である。
///
/// 別run・別sandbox・別状態で使い回せないよう`Clone`/`Copy`にしない。
#[derive(Debug)]
pub struct ProtectionConfirmation {
    pub(super) operation: DestructiveOperation,
    pub(super) sandbox: SandboxName,
    pub(super) fingerprint: ProtectionFingerprint,
}
