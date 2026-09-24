use std::time::{Duration, Instant};

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{
    CommandOutcome, OutputPolicy, TerminalCommand, WAIT_POLL_INTERVAL, configure, outcome,
    run_terminal_inner, spawn, unwaitable,
};

/// 端末を引き渡したcommandを実行し、終わるのを待つあいだ`every`ごとに`tick`を呼ぶ。
///
/// `tick`はこのthreadで走る。端末は子が持っているため、`tick`は端末へ何も書かない
/// ものとする。`tick`が長くかかれば、子の終了に気付くのもその分だけ遅れる。端末を
/// 引き渡さないcommandは`tick`を呼ばずに実行する。
pub(super) fn run_terminal_ticking(
    command: &TerminalCommand,
    output: &mut dyn ExternalOutput,
    every: Duration,
    tick: &mut dyn FnMut(),
) -> Result<CommandOutcome> {
    let spec = command.spec();
    if spec.output() != OutputPolicy::HandOver {
        return run_terminal_inner(command, output);
    }
    let mut process = configure(spec)?;
    output.hand_over();
    let mut child = spawn(&mut process, spec)?;
    let mut next = Instant::now() + every;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= next => {
                tick();
                next = Instant::now() + every;
            }
            Ok(None) => std::thread::sleep(WAIT_POLL_INTERVAL),
            Err(error) => break Err(unwaitable(&mut child, spec, &error)),
        }
    };
    output.finished();
    Ok(outcome(spec, status?, Vec::new(), Vec::new()))
}
