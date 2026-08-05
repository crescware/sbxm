use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, TerminalCommand, run_terminal_inner};

/// 出力が端末まで届く外部commandを実行する。
pub fn run_with_terminal(
    command: &TerminalCommand,
    output: &mut dyn ExternalOutput,
) -> Result<CommandOutcome> {
    run_terminal_inner(command, output)
}
