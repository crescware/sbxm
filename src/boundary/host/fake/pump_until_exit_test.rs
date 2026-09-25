use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::time::Duration;

use crate::boundary::host::{CommandSpec, TerminalCommand};
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
        None::<InputFeed<'_, std::fs::File>>,
        stdout,
        stderr,
        &mut |stream, bytes| {
            match stream {
                Stream::Stdout => out.extend_from_slice(bytes),
                Stream::Stderr => err.extend_from_slice(bytes),
            }
            Ok(())
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
    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| Ok(()))
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
    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| Ok(()))
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

    let error = pump_until_exit(&mut child, &spec(), None, Some(&signal), &mut |_, _| Ok(()))
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
    assert!(
        drain_pipe(&mut absent, &mut |bytes| {
            collected.extend_from_slice(bytes);
            Ok(())
        })
        .is_ok()
    );

    let mut pipe = Some(ScriptedPipe::new([
        ReadStep::Interrupted,
        ReadStep::Bytes(b"out"),
        ReadStep::WouldBlock,
    ])?);
    assert!(
        drain_pipe(&mut pipe, &mut |bytes| {
            collected.extend_from_slice(bytes);
            Ok(())
        })
        .is_ok()
    );
    assert!(
        pipe.is_some(),
        "an interrupted read leaves the pipe available"
    );
    assert_eq!(collected, b"");

    assert!(
        drain_pipe(&mut pipe, &mut |bytes| {
            collected.extend_from_slice(bytes);
            Ok(())
        })
        .is_ok()
    );
    assert_eq!(collected, b"out");
    assert!(pipe.is_some(), "a read that would block leaves the pipe");

    let mut ended = Some(ScriptedPipe::new([ReadStep::Bytes(b"last")])?);
    assert!(
        drain_pipe(&mut ended, &mut |bytes| {
            collected.extend_from_slice(bytes);
            Ok(())
        })
        .is_ok()
    );
    assert_eq!(collected, b"outlast");
    assert!(ended.is_none(), "a pipe that reached its end is closed");

    let mut failed = Some(ScriptedPipe::new([ReadStep::Failed])?);
    assert!(
        drain_pipe(&mut failed, &mut |bytes| {
            collected.extend_from_slice(bytes);
            Ok(())
        })
        .is_err()
    );
    Ok(())
}

#[test]
fn poll_pipes_accepts_missing_streams() -> Checked {
    assert_eq!(
        poll_pipes::<ChildStdout, ChildStderr>(None, None, None)
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
        None::<InputFeed<'_, std::fs::File>>,
        ScriptedPipe::new([ReadStep::Bytes(b"out")])?,
        ScriptedPipe::new([ReadStep::Bytes(b"err")])?,
        &mut |_, bytes| {
            output.relay(bytes);
            Ok(())
        },
    )
    .required_because("a child that has exited ends the relay")?;

    assert!(status.success());
    assert_eq!(output.text(), "outerr");
    Ok(())
}

// --- stdinへ渡すbyte列 ---

/// 書き込みの答えを順に返すstdin。
/// 書けた量をtestが決める書き込み端。書けたbyteと、閉じられたかどうかを残す。
///
/// pollが問い合わせる相手は本物のfdのままにする。`/dev/null`は常に書き込み可能と答える。
struct ScriptedWriter {
    ready: std::fs::File,
    answers: Vec<std::io::Result<usize>>,
    written: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    closed: std::rc::Rc<std::cell::Cell<bool>>,
}

impl ScriptedWriter {
    fn answering(answers: Vec<std::io::Result<usize>>) -> Checked<ScriptedWriter> {
        Ok(ScriptedWriter {
            ready: std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .required_because("a descriptor that always polls writable")?,
            answers: answers.into_iter().rev().collect(),
            written: std::rc::Rc::default(),
            closed: std::rc::Rc::default(),
        })
    }
}

impl std::io::Write for ScriptedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.answers.pop() {
            Some(Ok(accepted)) => {
                let accepted = accepted.min(bytes.len());
                self.written
                    .borrow_mut()
                    .extend_from_slice(&bytes[..accepted]);
                Ok(accepted)
            }
            Some(Err(error)) => Err(error),
            None => Ok(0),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl AsFd for ScriptedWriter {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.ready.as_fd()
    }
}

impl Drop for ScriptedWriter {
    fn drop(&mut self) {
        self.closed.set(true);
    }
}

#[test]
fn input_is_written_a_piece_at_a_time_and_closed_once_it_is_all_written() -> Checked {
    use std::io::{Error as IoError, ErrorKind};

    let writer = ScriptedWriter::answering(vec![
        Ok(2),
        Err(IoError::from(ErrorKind::WouldBlock)),
        Err(IoError::from(ErrorKind::Interrupted)),
        Ok(3),
    ])?;
    let written = std::rc::Rc::clone(&writer.written);
    let closed = std::rc::Rc::clone(&writer.closed);
    let mut feed = InputFeed::new(writer, b"hello");
    // 子がまだ読んでいない間は、書けた分だけ進めて戻る。
    feed.feed().required()?;
    assert_eq!(written.borrow().as_slice(), b"he");
    assert!(feed.waiting().is_some(), "the rest is still waiting");
    feed.feed().required()?;
    feed.feed().required()?;
    // 残りを書き終えたら書き込み端を閉じ、それ以上は書かない。
    feed.feed().required()?;
    assert_eq!(written.borrow().as_slice(), b"hello");
    assert!(closed.get(), "the child sees the end of its input");
    assert!(feed.waiting().is_none(), "nothing is left to wait for");
    feed.feed().required()?;
    Ok(())
}

#[test]
fn a_child_that_stopped_reading_ends_the_input_without_an_error() -> Checked {
    use std::io::{Error as IoError, ErrorKind};

    let writer = ScriptedWriter::answering(vec![Err(IoError::from(ErrorKind::BrokenPipe))])?;
    let closed = std::rc::Rc::clone(&writer.closed);
    let mut feed = InputFeed::new(writer, b"hello");
    // 結果は子の終了statusが決める。書けなかったことだけで実行を失敗にしない。
    feed.feed().required()?;
    assert!(
        closed.get(),
        "a child that stopped reading gets no more input"
    );
    feed.feed().required()?;

    // 何も受け付けない書き込み端も、次に書ける時まで待つだけである。
    let stalled_writer = ScriptedWriter::answering(Vec::new())?;
    let stalled_closed = std::rc::Rc::clone(&stalled_writer.closed);
    let mut stalled = InputFeed::new(stalled_writer, b"hello");
    stalled.feed().required()?;
    assert!(
        !stalled_closed.get(),
        "the input stays open until it is written"
    );
    Ok(())
}

#[test]
fn input_that_cannot_be_written_ends_the_child_and_keeps_what_the_os_said() -> Checked {
    use std::io::Error as IoError;

    let mut child = waiting_child()?;
    let pid = rustix::process::Pid::from_child(&child);
    let signal = signal()?;
    let writer =
        ScriptedWriter::answering(vec![Err(IoError::other("the pipe could not be written"))])?;

    let error = pump(
        &mut child,
        &spec(),
        None,
        Some(&signal),
        Some(InputFeed::new(writer, b"hello")),
        ScriptedPipe::new([])?,
        ScriptedPipe::new([])?,
        &mut |_, _| Ok(()),
    )
    .refused_because("input that cannot be written refuses the run")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandInputUnwritable)
    );
    assert_eq!(cause(&error)?, "the pipe could not be written");
    assert!(is_collected(pid), "the child does not outlive its input");
    Ok(())
}

#[test]
fn input_for_a_child_started_without_an_input_pipe_is_refused() -> Checked {
    let mut child = child(Stdio::piped(), Stdio::piped())?;
    let pid = rustix::process::Pid::from_child(&child);
    let signal = signal()?;
    let spec = spec().with_input(b"hello".to_vec());

    let error = pump_until_exit(&mut child, &spec, None, Some(&signal), &mut |_, _| Ok(()))
        .refused_because("there is nowhere to write the input")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandInputUnwritable)
    );
    assert!(is_collected(pid));
    Ok(())
}
