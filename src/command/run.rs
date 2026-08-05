use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, run_inner};

/// 外部commandを実行し、出力をcaptureする。
pub fn run(spec: &CommandSpec) -> Result<CommandOutcome> {
    run_inner(spec, spec.timeout.duration())
}
