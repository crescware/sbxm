use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::time::Duration;

use crate::command::{CommandSpec, TerminalCommand};
use crate::design::ExternalOutput;
use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::testing::command::{ReadStep, ScriptedPipe};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

fn child(stdout: Stdio, stderr: Stdio) -> Checked<Child> {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    command
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .required_because("the test child must start")
}

/// 終わりを待つだけの子。読む相手はtestが渡すため、pipeは要らない。
fn waiting_child() -> Checked<Child> {
    child(Stdio::null(), Stdio::null())
}

/// 既に終わり、終了statusも引き取った子。
///
/// `wait`が控えた終了statusを`try_wait`はそのまま返すため、「子が終わったあと」の分岐は
/// 待ち時間に左右されずに決まる。
fn finished_child() -> Checked<Child> {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 0"]);
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .required_because("the test child must start")?;
    child
        .wait()
        .required_because("the test child must finish")?;
    Ok(child)
}

/// `pump`が渡したbyteを、2本に分けて受け取る。
fn capture<O: Read + AsFd, E: Read + AsFd>(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: Option<&SignalGuard>,
    stdout: O,
    stderr: E,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status = pump(
        child,
        spec,
        limit,
        signal,
        stdout,
        stderr,
        &mut |stream, bytes| match stream {
            Stream::Stdout => out.extend_from_slice(bytes),
            Stream::Stderr => err.extend_from_slice(bytes),
        },
    )?;
    Ok((status, out, err))
}

fn spec() -> CommandSpec {
    CommandSpec::probe("sh", &["-c", "sleep 30"])
}

fn signal() -> Checked<SignalGuard> {
    SignalGuard::new().required_because("SIGINT registration must succeed")
}

/// 子processが既に回収されているか。
fn is_collected(pid: rustix::process::Pid) -> bool {
    rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).err()
        == Some(rustix::io::Errno::CHILD)
}

/// 診断が持つ原因の原文。
fn cause(error: &Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the refusal states what could not be read")
}

/// 診断が持つ事実の項目名。
fn labels(error: &Error) -> Checked<Vec<String>> {
    Ok(error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .map(|fact| fact.label().id.to_string())
        .collect())
}

#[test]
fn a_missing_stdout_pipe_is_reported() -> Checked {
    let mut child = child(Stdio::null(), Stdio::piped())?;
    let signal = signal()?;
    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| {})
        .refused_because("a missing stdout pipe must be refused")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    Ok(())
}

#[test]
fn a_missing_stderr_pipe_is_reported() -> Checked {
    let mut child = child(Stdio::piped(), Stdio::null())?;
    let signal = signal()?;
    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| {})
        .refused_because("a missing stderr pipe must be refused")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    Ok(())
}

#[test]
fn an_interrupted_capture_is_canceled_before_waiting() -> Checked {
    let mut child = child(Stdio::piped(), Stdio::piped())?;
    let signal =
        SignalGuard::interrupted_for_test().required_because("SIGINT registration must succeed")?;

    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| {})
        .refused_because("an interrupted capture must be canceled")?;

    assert!(matches!(error, Error::Canceled));
    Ok(())
}

#[test]
fn an_interrupt_that_arrived_before_the_output_was_read_leaves_no_child_behind() -> Checked {
    let mut child = waiting_child()?;
    let pid = rustix::process::Pid::from_child(&child);
    let signal =
        SignalGuard::interrupted_for_test().required_because("SIGINT registration must succeed")?;

    let error = capture(
        &mut child,
        &spec(),
        None,
        Some(&signal),
        ScriptedPipe::new([])?,
        ScriptedPipe::new([])?,
    )
    .refused_because("an interrupted capture must be canceled")?;

    assert!(matches!(error, Error::Canceled));
    // 中断は「何も変えていない」ことの表明であり、動いたままの子はその表明を破る。
    assert!(
        is_collected(pid),
        "a canceled capture must not leave the child running"
    );
    Ok(())
}

#[test]
fn an_interrupt_that_arrives_while_the_output_is_read_ends_the_same_run() -> Checked {
    let mut child = waiting_child()?;
    let pid = rustix::process::Pid::from_child(&child);
    let signal = signal()?;
    // 読んでいる最中に届くCtrl-Cは、実行のどの時点にも入り込む。読みの途中で届いた回も、
    // 読み終えてから届いた回と同じ終わり方をする。
    let stdout = ScriptedPipe::new([ReadStep::Bytes(b"partial")])?
        .interrupting(signal.interrupt_switch_for_test());

    let error = capture(
        &mut child,
        &spec(),
        None,
        Some(&signal),
        stdout,
        ScriptedPipe::new([])?,
    )
    .refused_because("an interrupt during the read must cancel the capture")?;

    assert!(matches!(error, Error::Canceled));
    assert!(
        is_collected(pid),
        "a canceled capture must not leave the child running"
    );
    Ok(())
}

#[test]
fn a_stream_that_cannot_be_read_ends_the_child_and_keeps_what_the_os_said() -> Checked {
    // 読めなくなったのがstdoutでもstderrでも、実行は成立せず、子も残さない。
    for stderr_fails in [false, true] {
        let mut child = waiting_child()?;
        let pid = rustix::process::Pid::from_child(&child);
        let signal = signal()?;
        let failing = ScriptedPipe::new([ReadStep::Failed])?;
        let ended = ScriptedPipe::new([])?;
        let (stdout, stderr) = if stderr_fails {
            (ended, failing)
        } else {
            (failing, ended)
        };

        let error = capture(&mut child, &spec(), None, Some(&signal), stdout, stderr)
            .refused_because("a stream that cannot be read must refuse the run")?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandOutputUnreadable)
        );
        assert_eq!(
            labels(&error)?,
            vec!["diagnostic-command-label", "diagnostic-cause-label"],
            "the invocation and the reason the OS gave are both kept"
        );
        assert_eq!(cause(&error)?, "the pipe could not be read");
        assert!(
            is_collected(pid),
            "the child must not outlive the output it was writing to"
        );
    }
    Ok(())
}

#[test]
fn the_bytes_that_arrive_after_the_child_exits_are_still_collected() -> Checked {
    let mut child = finished_child()?;
    let signal = signal()?;
    // 子が終わったことに気付く前には読めなかったbyteが、pipeに残っている。
    let stdout = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::Bytes(b"tail")])?;
    let stderr = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::Bytes(b"late")])?;

    let (status, stdout_bytes, stderr_bytes) =
        capture(&mut child, &spec(), None, Some(&signal), stdout, stderr)
            .required_because("a child that has exited ends the capture")?;

    assert!(status.success());
    assert_eq!(stdout_bytes, b"tail");
    assert_eq!(stderr_bytes, b"late");
    Ok(())
}

#[test]
fn a_stream_that_cannot_be_read_after_the_child_exits_still_refuses_the_run() -> Checked {
    // 終了statusを手にしていても、出力を最後まで読めなければ結果は使えない。
    for stderr_fails in [false, true] {
        let mut child = finished_child()?;
        let signal = signal()?;
        let failing = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::Failed])?;
        let blocked = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::WouldBlock])?;
        let (stdout, stderr) = if stderr_fails {
            (blocked, failing)
        } else {
            (failing, blocked)
        };

        let error = capture(&mut child, &spec(), None, Some(&signal), stdout, stderr)
            .refused_because("output that cannot be read must refuse the run")?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandOutputUnreadable)
        );
        assert_eq!(cause(&error)?, "the pipe could not be read");
    }
    Ok(())
}

#[test]
fn a_capture_that_passed_its_deadline_ends_the_child_and_names_the_limit() -> Checked {
    let mut child = waiting_child()?;
    let pid = rustix::process::Pid::from_child(&child);
    let signal = signal()?;
    // 期限0は「既に過ぎている」。待ち時間で結果が変わらないため、判定は毎回同じになる。
    let error = capture(
        &mut child,
        &spec(),
        Some(Duration::ZERO),
        Some(&signal),
        ScriptedPipe::new([ReadStep::WouldBlock])?,
        ScriptedPipe::new([ReadStep::WouldBlock])?,
    )
    .refused_because("a capture past its deadline must be refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert!(
        diagnostic
            .description
            .args
            .contains(&("program", "sh".to_string())),
        "the report names the command that was cut off: {:?}",
        diagnostic.description
    );
    assert!(
        diagnostic
            .description
            .args
            .contains(&("seconds", "0".to_string())),
        "the report names the limit that was exceeded: {:?}",
        diagnostic.description
    );
    assert!(
        is_collected(pid),
        "the child must already be collected when the timeout is reported"
    );
    Ok(())
}

#[test]
fn a_child_reaped_before_capture_is_reported_as_a_wait_failure() -> Checked {
    let mut child = waiting_child()?;
    rustix::process::waitpid(
        Some(rustix::process::Pid::from_child(&child)),
        rustix::process::WaitOptions::empty(),
    )
    .required_because("the test child must be reaped")?;
    let signal = signal()?;

    let error = capture(
        &mut child,
        &spec(),
        None,
        Some(&signal),
        ScriptedPipe::new([])?,
        ScriptedPipe::new([])?,
    )
    .refused_because("a child that cannot be waited must be refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"],
        "the invocation and the reason the OS gave are both kept"
    );
    Ok(())
}

#[test]
fn drain_pipe_handles_empty_and_read_errors() -> Checked {
    let mut collected = Vec::new();
    let mut absent: Option<ScriptedPipe> = None;
    assert!(drain_pipe(&mut absent, &mut |bytes| collected.extend_from_slice(bytes)).is_ok());

    let mut pipe = Some(ScriptedPipe::new([
        ReadStep::Interrupted,
        ReadStep::Bytes(b"out"),
        ReadStep::WouldBlock,
    ])?);
    assert!(drain_pipe(&mut pipe, &mut |bytes| collected.extend_from_slice(bytes)).is_ok());
    assert!(
        pipe.is_some(),
        "an interrupted read leaves the pipe available"
    );
    assert_eq!(collected, b"");

    assert!(drain_pipe(&mut pipe, &mut |bytes| collected.extend_from_slice(bytes)).is_ok());
    assert_eq!(collected, b"out");
    assert!(pipe.is_some(), "a read that would block leaves the pipe");

    let mut ended = Some(ScriptedPipe::new([ReadStep::Bytes(b"last")])?);
    assert!(drain_pipe(&mut ended, &mut |bytes| collected.extend_from_slice(bytes)).is_ok());
    assert_eq!(collected, b"outlast");
    assert!(ended.is_none(), "a pipe that reached its end is closed");

    let mut failed = Some(ScriptedPipe::new([ReadStep::Failed])?);
    assert!(drain_pipe(&mut failed, &mut |bytes| collected.extend_from_slice(bytes)).is_err());
    Ok(())
}

#[test]
fn poll_pipes_accepts_missing_streams() -> Checked {
    assert_eq!(
        poll_pipes::<ChildStdout, ChildStderr>(None, None)
            .required_because("polling no pipes must work")?,
        (false, false)
    );
    Ok(())
}

#[test]
fn a_relay_hands_both_streams_to_one_receiver_in_the_order_they_arrive() -> Checked {
    let mut child = finished_child()?;
    let relayed = TerminalCommand::relayed("sh", &["-c", "exit 0"]);
    let mut output = RecordedOutput::new();

    // 中継では、どちらのstreamから届いたbyteかを分けない。読めた順がそのまま画面の順になる。
    let status = pump(
        &mut child,
        relayed.spec(),
        None,
        None,
        ScriptedPipe::new([ReadStep::Bytes(b"out")])?,
        ScriptedPipe::new([ReadStep::Bytes(b"err")])?,
        &mut |_, bytes| output.relay(bytes),
    )
    .required_because("a child that has exited ends the relay")?;

    assert!(status.success());
    assert_eq!(output.text(), "outerr");
    Ok(())
}
