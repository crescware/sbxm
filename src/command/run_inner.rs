use std::os::unix::process::CommandExt;
use std::process::{Child, ExitStatus};
use std::time::Duration;

use crate::diagnostics::Result;

use super::{
    CommandOutcome, CommandSpec, SignalGuard, Stream, configure, outcome, pump_until_exit, spawn,
    spawn_failure,
};

/// 出力をcaptureして実行する。
///
/// 捕捉した出力を読むのはsbxmだけなので、利用者の端末はこの実行の影響を受けない。
pub(super) fn run_inner(spec: &CommandSpec, limit: Option<Duration>) -> Result<CommandOutcome> {
    let mut command = configure(spec);

    // Capture commandを専用のprocess groupへ置く。ただし打ち切りでgroupへsignalは送らず、
    // `terminate_child`は直接の子だけを終わらせる。専用groupの目的は、端末からforeground
    // groupへ届くCtrl-Cが、このcommandの子孫（daemonを含みうる）へ到達するのを防ぐこと。
    command.process_group(0);

    // SIGINTはsigactionが拒むsignalではなく、既に手当てされたsignalへ2つ目のactionを足す
    // ときはsyscallも呼ばない。それでも失敗を握り潰さない。
    let signal = SignalGuard::new().map_err(|error| spawn_failure(spec, &error))?;

    let mut child = spawn(&mut command, spec)?;
    let (status, stdout, stderr) = capture(&mut child, spec, limit, &signal)?;
    Ok(outcome(spec, status, stdout, stderr))
}

/// 2本のstreamを、それぞれ別のbyte列として引き取る。
fn capture(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: &SignalGuard,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = pump_until_exit(
        child,
        spec,
        limit,
        Some(signal),
        &mut |stream, bytes| match stream {
            Stream::Stdout => stdout.extend_from_slice(bytes),
            Stream::Stderr => stderr.extend_from_slice(bytes),
        },
    )?;
    Ok((status, stdout, stderr))
}
