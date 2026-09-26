use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{
    CommandOutcome, HidingLines, OutputPolicy, TerminalCommand, configure, outcome, run_relay,
    spawn, wait_with_limit,
};

/// 出力が端末まで届くcommandを実行する。
///
/// 端末へ出る2つのpolicyは、sbxmと同じprocess groupに残す。別のgroupへ移すと利用者の
/// Ctrl-Cが子processへ届かず、sbxmが終わったあとも端末へ書き続ける相手が残る。そのため、
/// これらのtimeoutで終わらせるのは直接の子だけとする。
///
/// `tick`は端末を引き渡したcommandを待つあいだだけ、間隔ごとに呼ぶ。端末は子が持って
/// いるため、`tick`は端末へ何も書かない手続きとする。出力を中継するcommandでは呼ばない。
pub(super) fn run_terminal_inner(
    command: &TerminalCommand,
    output: &mut dyn ExternalOutput,
    tick: Option<(Duration, &mut dyn FnMut())>,
) -> Result<CommandOutcome> {
    let spec = command.spec();
    let limit = spec.timeout.duration();
    let mut process = configure(spec);

    match spec.output() {
        // 何を書くか観測できないため、境界の空行を先に置く。
        OutputPolicy::HandOver => {
            output.hand_over();
            let mut child = spawn(&mut process, spec)?;
            let status = wait_with_limit(&mut child, spec, limit, tick);
            output.finished();
            Ok(outcome(spec, status?, Vec::new(), Vec::new()))
        }
        OutputPolicy::Capture | OutputPolicy::Relay => {
            let mut child = spawn(&mut process, spec)?;
            let mut hiding = HidingLines::new(output, command.hidden());
            let status = run_relay(&mut child, spec, limit, &mut hiding);
            hiding.finished();
            Ok(outcome(spec, status?, Vec::new(), Vec::new()))
        }
    }
}
