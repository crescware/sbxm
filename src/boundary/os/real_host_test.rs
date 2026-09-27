//! 配線のtest: `RealHost`の各methodが、OSの基本操作と`SystemClock`を判断のcodeへ渡すこと。
//!
//! 判断そのものは`boundary::host`のtestが台本のOSで確かめる。ここは実物の組み合わせが1度
//! 通ることだけを見る。どのcommandも自ら終わり、途中の状態を待たない。

use std::time::Duration;

use crate::boundary::host::{CommandSpec, HostEnvironment, PtyConfirmedCommand, TerminalCommand};
use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::RealHost;

#[test]
fn the_real_host_captures_both_streams() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf out; printf err >&2"]);
    let outcome = RealHost.run(&spec).required()?;
    assert!(outcome.success());
    assert_eq!(outcome.stdout, b"out");
    assert_eq!(outcome.stderr, b"err");
    Ok(())
}

#[test]
fn the_real_host_streams_stdout_itself() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf received"]);
    let mut sink = Vec::new();
    RealHost.run_streaming(&spec, &mut sink, 1024).required()?;
    assert_eq!(sink, b"received");
    Ok(())
}

#[test]
fn the_real_host_relays_what_the_command_writes() -> Checked {
    let command = TerminalCommand::relayed("sh", &["-c", "printf progress"]);
    let mut output = RecordedOutput::new();
    let outcome = RealHost
        .run_with_terminal(&command, &mut output)
        .required()?;
    assert!(outcome.success());
    assert_eq!(output.text(), "progress");
    assert_eq!(output.finished, 1);
    Ok(())
}

#[test]
fn the_real_host_hands_the_terminal_over_while_it_waits() -> Checked {
    let command = TerminalCommand::handed_over("sh", &["-c", "exit 0"]);
    let mut output = RecordedOutput::new();
    let mut ticks = 0;
    let outcome = RealHost
        .run_with_terminal_ticking(
            &command,
            &mut output,
            Duration::from_secs(3600),
            &mut || ticks += 1,
        )
        .required()?;
    assert!(outcome.success());
    assert_eq!((output.handed_over, output.finished), (1, 1));
    assert_eq!(ticks, 0);
    Ok(())
}

#[test]
fn the_real_host_uses_the_pty_runner() -> Checked {
    let error = RealHost
        .run_pty_confirmed(&PtyConfirmedCommand::new(
            "/does/not/exist/sbx",
            &[],
            "the sandbox",
            "confirmation",
        ))
        .refused_because("the real host delegates PTY execution")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}
