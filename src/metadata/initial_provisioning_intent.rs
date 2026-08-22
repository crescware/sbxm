/// 初回構築の開始から完了まで保持する、復旧対象の世代。
///
/// このintentが存在する間は、初回構築が一度はhostまたはSandboxを変更した可能性が
/// ある。通常の`prepare`はそれを暗黙に継続せず、利用者が明示した`repair`だけがこの
/// 世代を対象に復旧する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProvisioningIntent {
    pub target_dockerfile_sha256: String,
}
