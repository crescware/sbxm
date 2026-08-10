use crate::command::{CommandOutcome, CommandSpec, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::diagnose_failure;

/// 名前が一致するimageが存在するか。
///
/// 一覧の失敗は不在へ丸めず、そのまま呼び出し側の失敗にする。
pub fn exists(host: &dyn HostEnvironment, name: &str) -> Result<bool> {
    let spec = CommandSpec::capture("docker", &["image", "ls", "--quiet", name])
        .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host
        .run(&spec)
        .and_then(CommandOutcome::require_success)
        .map_err(|error| diagnose_failure(host, error))?;
    Ok(!outcome.stdout_text().trim().is_empty())
}
