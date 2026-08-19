/// 初回構築の開始から完了まで保持する、復旧対象の世代。
///
/// このintentが存在する間は、初回構築が一度はhostまたはSandboxを変更した可能性が
/// ある。通常の`prepare`はこの世代を対象に同じgeneration選択規則で再開し、全成果物が
/// 揃ったことを証明できた時点でこのintentを消す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProvisioningIntent {
    pub target_dockerfile_sha256: String,
}
