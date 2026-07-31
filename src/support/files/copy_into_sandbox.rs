use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::copy_steps;

/// 一時fileを経由して配置し、成功・失敗のどちらでも一時fileを削除する。
pub(super) fn copy_into_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    index: usize,
    source: &Path,
    destination: &str,
) -> Result<()> {
    let staged = format!("/tmp/sbxm-file-{index}");
    let pending = format!("{destination}.sbxm-new");
    let result = copy_steps(host, sandbox, source, &staged, destination, &pending);

    let _ = sandbox::exec_as_root(host, sandbox, &["rm", "-f", &staged, &pending]);
    result
}
