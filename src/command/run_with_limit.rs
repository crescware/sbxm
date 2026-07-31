use std::time::Duration;

use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, run_inner};

/// timeout classの既定値ではない待ち時間で実行する。
///
/// 最短のclassでも10秒あるため、deadlineに達する側の分岐はtestからしか踏めない。
#[cfg(test)]
pub fn run_with_limit(spec: &CommandSpec, limit: Duration) -> Result<CommandOutcome> {
    run_inner(spec, Some(limit))
}
