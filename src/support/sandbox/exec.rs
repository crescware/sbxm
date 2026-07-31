use crate::command::{CommandOutcome, HostEnvironment};
use crate::diagnostics::Result;

use super::run_exec;

/// Sandbox内でcommandを実行する。
///
/// 引数配列のまま渡し、shellを介さない。出力はparseまたは秘匿のためcaptureする。
pub fn exec(host: &dyn HostEnvironment, sandbox: &str, args: &[&str]) -> Result<CommandOutcome> {
    run_exec(host, sandbox, None, args)
}
