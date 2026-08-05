use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{
    CommandOutcome, OutputPolicy, TerminalCommand, configure, outcome, run_relay, spawn,
    wait_with_limit,
};

/// 出力が端末まで届くcommandを実行する。
///
/// 端末へ出る2つのpolicyは、sbxmと同じprocess groupに残す。別のgroupへ移すと利用者の
/// Ctrl-Cが子processへ届かず、sbxmが終わったあとも端末へ書き続ける相手が残る。そのため、
/// これらのtimeoutで終わらせるのは直接の子だけとする。
pub(super) fn run_terminal_inner(
    command: &TerminalCommand,
    output: &mut dyn ExternalOutput,
) -> Result<CommandOutcome> {
    let spec = command.spec();
    let limit = spec.timeout.duration();
    let mut process = configure(spec);

    match spec.output() {
        // 何を書くか観測できないため、境界の空行を先に置く。
        OutputPolicy::HandOver => {
            output.hand_over();
            let mut child = spawn(&mut process, spec)?;
            let status = wait_with_limit(&mut child, spec, limit);
            output.finished();
            Ok(outcome(spec, status?, Vec::new(), Vec::new()))
        }
        OutputPolicy::Capture | OutputPolicy::Relay => {
            let mut child = spawn(&mut process, spec)?;
            let status = run_relay(&mut child, spec, limit, output);
            output.finished();
            Ok(outcome(spec, status?, Vec::new(), Vec::new()))
        }
    }
}
