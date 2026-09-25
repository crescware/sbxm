use std::io::Write;
use std::os::unix::process::CommandExt;

use crate::diagnostics::Result;

use super::{
    CommandOutcome, CommandSpec, MAX_KEPT_STDERR, SignalGuard, Stream, configure, outcome,
    output_too_large, pump_until_exit, spawn, spawn_failure, unstored,
};

/// 出力をcaptureして実行し、stdoutだけを届いた順に`sink`へ流す。
///
/// stdoutを溜めないため、大きな出力でもmemoryを使い切らない。`limit`byteを超えた時点で
/// 子を終わらせ、それ以上は受け取らない。stderrは診断に使うため、上限までだけ溜める。
/// `sink`へ書けなかった場合は、出力を読めなかったのではなく、hostで残せなかったと報告する。
pub(super) fn run_streaming(
    spec: &CommandSpec,
    sink: &mut dyn Write,
    limit: u64,
) -> Result<CommandOutcome> {
    let mut command = configure(spec)?;
    // `run_inner`と同じく専用のprocess groupへ置き、端末からのCtrl-Cを子孫へ届けない。
    command.process_group(0);
    let signal = SignalGuard::new().map_err(|error| spawn_failure(spec, &error))?;
    let mut child = spawn(&mut command, spec)?;

    let mut received: u64 = 0;
    let mut exceeded = false;
    let mut unstored_cause = None;
    let mut stderr = Vec::new();
    let status = pump_until_exit(
        &mut child,
        spec,
        spec.timeout.duration(),
        Some(&signal),
        &mut |stream, bytes| match stream {
            Stream::Stdout => {
                received = received.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
                if received > limit {
                    exceeded = true;
                    return Err(std::io::Error::other("the output exceeded its limit"));
                }
                let written = sink.write_all(bytes);
                if let Err(error) = &written {
                    unstored_cause = Some(error.to_string());
                }
                written
            }
            Stream::Stderr => {
                let room = MAX_KEPT_STDERR.saturating_sub(stderr.len());
                stderr.extend_from_slice(&bytes[..bytes.len().min(room)]);
                Ok(())
            }
        },
    );
    let status = match (status, unstored_cause) {
        (Ok(status), _) => status,
        (Err(_), _) if exceeded => return Err(output_too_large(spec, limit)),
        (Err(_), Some(cause)) => return Err(unstored(spec, &cause)),
        (Err(error), None) => return Err(error),
    };
    if let Err(error) = sink.flush() {
        return Err(unstored(spec, &error.to_string()));
    }
    Ok(outcome(spec, status, Vec::new(), stderr))
}
