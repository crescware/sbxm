use crate::project::SandboxName;

use super::super::{Assessment, DestructiveOperation, ProtectionSnapshot};

/// Sandboxがそもそも無い案件の観測結果。
///
/// 観測する対象が無いことと、観測できないことは別である。前者はここで空の
/// `Assessment`として表し、[`super::assess`]が返す観測結果と同じ型で扱う。
///
/// Sandboxがある場合の観測は必ず`ConfirmableLoss::SandboxWritableLayer`を1件持つため、
/// この空の観測結果とfingerprintが一致することはない。確認から削除までのあいだに
/// Sandboxが現れた場合は、その差がfingerprintに出る。
pub fn assess_absent(
    operation: DestructiveOperation,
    project: String,
    sandbox: &SandboxName,
) -> ProtectionSnapshot {
    ProtectionSnapshot::new(Assessment::empty(operation, project, sandbox.clone()))
}
