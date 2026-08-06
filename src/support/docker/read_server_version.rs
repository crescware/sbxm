use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::version_probe;

/// `docker version`が返すserver側versionの文字列。
///
/// 非ゼロ終了は`ErrorId::ExternalCommandFailed`として返る。到達可否そのものを診断に
/// 変換するかどうかは呼び出し側が決める。
pub fn read_server_version(host: &dyn HostEnvironment) -> Result<String> {
    Ok(version_probe(host)?.require_success()?.stdout_text())
}
