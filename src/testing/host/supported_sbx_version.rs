use crate::boundary::host::protocol::MINIMUM_CLI_VERSION;

/// `sbx version`へ、このbuildが求める最低versionのCLIが返す出力。
///
/// 実物（v0.48.0）と同じく、versionの後にbuildのcommitが続く。最低versionを上げても、
/// fakeの答えはここだけが追従する。
pub fn supported_sbx_version() -> String {
    format!("sbx version v{MINIMUM_CLI_VERSION} 5caf923e0587d376848f6f388eca3e0f5ca00e2d\n")
}
