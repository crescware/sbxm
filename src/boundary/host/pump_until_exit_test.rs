use std::io::{self, ErrorKind};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::Duration;

use crate::boundary::host::{CommandSpec, TerminalCommand, run_inner, run_relay};
use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::testing::command::{
    End, Event, ReadStep, ScriptedOs, ScriptedPipe, ScriptedWriter, Step,
};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

fn spec() -> CommandSpec {
    CommandSpec::probe("fake-tool", &[])
}

/// `code`で自ら終わった子の終了status。
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

/// 別の場所で引き取られた子を尋ねたときの失敗。
fn no_child() -> io::Error {
    io::Error::from_raw_os_error(rustix::io::Errno::CHILD.raw_os_error())
}

/// 割り込まれない実行で`pump_until_exit`が渡したbyteを、2本に分けて受け取る。
fn capture(
    os: &ScriptedOs,
    spec: &CommandSpec,
    limit: Option<Duration>,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status = pump_until_exit(
        os,
        os.clock(),
        &mut (),
        spec,
        limit,
        &|| false,
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

/// 子を終わらせ、終了statusを引き取ったか。
fn ended(events: &[Event]) -> bool {
    events
        .windows(2)
        .any(|pair| pair == [Event::Ended, Event::WaitedExit])
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
    let os = ScriptedOs::default().without(End::Stdout);
    let error =
        capture(&os, &spec(), None).refused_because("a missing stdout pipe must be refused")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn a_missing_stderr_pipe_is_reported() -> Checked {
    let os = ScriptedOs::default().without(End::Stderr);
    let error =
        capture(&os, &spec(), None).refused_because("a missing stderr pipe must be refused")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn an_interrupted_capture_is_canceled_before_waiting() -> Checked {
    let os = ScriptedOs::default();
    let error = pump_until_exit(
        &os,
        os.clock(),
        &mut (),
        &spec(),
        None,
        &|| true,
        &mut |_, _| Ok(()),
    )
    .refused_because("an interrupted capture must be canceled")?;

    assert!(matches!(error, Error::Canceled));
    let events = os.events();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Watched(_) | Event::CheckedExit)),
        "{events:?}"
    );
    Ok(())
}

#[test]
fn an_interrupt_that_arrived_before_the_output_was_read_leaves_no_child_behind() -> Checked {
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::held_open([]))
        .stderr(ScriptedPipe::held_open([]));
    let error = pump_until_exit(
        &os,
        os.clock(),
        &mut (),
        &spec(),
        None,
        &|| true,
        &mut |_, _| Ok(()),
    )
    .refused_because("an interrupted capture must be canceled")?;

    assert!(matches!(error, Error::Canceled));
    // 中断は「何も変えていない」ことの表明であり、動いたままの子はその表明を破る。
    assert!(
        ended(&os.events()),
        "a canceled capture must not leave the child running"
    );
    Ok(())
}

#[test]
fn an_interrupt_that_arrives_while_the_output_is_read_ends_the_same_run() -> Checked {
    // 読んでいる最中に届くCtrl-Cは、実行のどの時点にも入り込む。待ちの途中で届いた回も、
    // 読み終えてから届いた回と同じ終わり方をする。
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::held_open([ReadStep::Bytes(b"partial")]))
        .interrupting_at_poll(1);

    let error = run_inner(&os, os.clock(), &spec(), None)
        .refused_because("an interrupt during the read must cancel the capture")?;

    assert!(matches!(error, Error::Canceled));
    let events = os.events();
    assert!(!events.contains(&Event::CheckedExit), "{events:?}");
    assert!(
        ended(&events),
        "a canceled capture must not leave the child running"
    );
    assert_eq!(events.last(), Some(&Event::StoppedWatching));
    Ok(())
}

#[test]
fn a_stream_that_cannot_be_read_ends_the_child_and_keeps_what_the_os_said() -> Checked {
    // 読めなくなったのがstdoutでもstderrでも、実行は成立せず、子も残さない。
    for stderr_fails in [false, true] {
        let failing = ScriptedPipe::new([ReadStep::Failed]);
        let closed = ScriptedPipe::new([]);
        let (stdout, stderr) = if stderr_fails {
            (closed, failing)
        } else {
            (failing, closed)
        };
        let os = ScriptedOs::default().stdout(stdout).stderr(stderr);

        let error = capture(&os, &spec(), None)
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
            ended(&os.events()),
            "the child must not outlive the output it was writing to"
        );
    }
    Ok(())
}

#[test]
fn the_bytes_that_arrive_after_the_child_exits_are_still_collected() -> Checked {
    // 子が終わったことに気付く前には読めなかったbyteが、pipeに残っている。
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([
            ReadStep::WouldBlock,
            ReadStep::Bytes(b"tail"),
        ]))
        .stderr(ScriptedPipe::new([
            ReadStep::WouldBlock,
            ReadStep::Bytes(b"late"),
        ]))
        .exits([Ok(Some(exited(0)))]);

    let (status, stdout, stderr) =
        capture(&os, &spec(), None).required_because("a child that has exited ends the capture")?;

    assert!(status.success());
    assert_eq!(stdout, b"tail");
    assert_eq!(stderr, b"late");
    assert!(!os.events().contains(&Event::Ended));
    Ok(())
}

#[test]
fn a_stream_that_cannot_be_read_after_the_child_exits_still_refuses_the_run() -> Checked {
    // 終了statusを手にしていても、出力を最後まで読めなければ結果は使えない。
    for stderr_fails in [false, true] {
        let failing = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::Failed]);
        let blocked = ScriptedPipe::new([ReadStep::WouldBlock, ReadStep::WouldBlock]);
        let (stdout, stderr) = if stderr_fails {
            (blocked, failing)
        } else {
            (failing, blocked)
        };
        let os = ScriptedOs::default()
            .stdout(stdout)
            .stderr(stderr)
            .exits([Ok(Some(exited(0)))]);

        let error = capture(&os, &spec(), None)
            .refused_because("output that cannot be read must refuse the run")?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandOutputUnreadable)
        );
        assert_eq!(cause(&error)?, "the pipe could not be read");
        assert!(
            !os.events().contains(&Event::Ended),
            "a child that has exited is not ended again"
        );
    }
    Ok(())
}

#[test]
fn a_capture_that_passed_its_deadline_ends_the_child_and_names_the_limit() -> Checked {
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::held_open([]))
        .stderr(ScriptedPipe::held_open([]));
    // 期限0は「既に過ぎている」。
    let error = capture(&os, &spec(), Some(Duration::ZERO))
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
            .contains(&("program", "fake-tool".to_string())),
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
        ended(&os.events()),
        "the child must already be collected when the timeout is reported"
    );
    Ok(())
}

#[test]
fn a_child_reaped_before_capture_is_reported_as_a_wait_failure() -> Checked {
    let os = ScriptedOs::default().exits([Err(no_child())]);

    let error = capture(&os, &spec(), None)
        .refused_because("a child that cannot be waited must be refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"],
        "the invocation and the reason the OS gave are both kept"
    );
    assert!(ended(&os.events()));
    Ok(())
}

#[test]
fn drain_pipe_handles_empty_and_read_errors() -> Checked {
    let mut collected = Vec::new();
    let mut absent: Option<ScriptedPipe> = None;
    drain_pipe(&mut absent, &mut |bytes| {
        collected.extend_from_slice(bytes);
        Ok(())
    })
    .required()?;

    let mut pipe = Some(ScriptedPipe::new([
        ReadStep::Interrupted,
        ReadStep::Bytes(b"out"),
        ReadStep::WouldBlock,
    ]));
    drain_pipe(&mut pipe, &mut |bytes| {
        collected.extend_from_slice(bytes);
        Ok(())
    })
    .required()?;
    assert!(
        pipe.is_some(),
        "an interrupted read leaves the pipe available"
    );
    assert_eq!(collected, b"");

    drain_pipe(&mut pipe, &mut |bytes| {
        collected.extend_from_slice(bytes);
        Ok(())
    })
    .required()?;
    assert_eq!(collected, b"out");
    assert!(pipe.is_some(), "a read that would block leaves the pipe");

    let mut ended = Some(ScriptedPipe::new([ReadStep::Bytes(b"last")]));
    drain_pipe(&mut ended, &mut |bytes| {
        collected.extend_from_slice(bytes);
        Ok(())
    })
    .required()?;
    assert_eq!(collected, b"outlast");
    assert!(ended.is_none(), "a pipe that reached its end is closed");

    let mut failed = Some(ScriptedPipe::new([ReadStep::Failed]));
    drain_pipe(&mut failed, &mut |bytes| {
        collected.extend_from_slice(bytes);
        Ok(())
    })
    .refused_because("a pipe that cannot be read")?;

    // 受け手が受け取れないと答えれば、読めなかった場合と同じく止まる。
    let mut refused = Some(ScriptedPipe::new([ReadStep::Bytes(b"more")]));
    let error = drain_pipe(&mut refused, &mut |_| Err(io::Error::other("full")))
        .refused_because("the receiver takes nothing")?;
    assert_eq!(error.to_string(), "full");
    Ok(())
}

#[test]
fn poll_pipes_accepts_missing_streams() -> Checked {
    let os = ScriptedOs::default();
    poll_pipes(&os, None, None, None).required_because("polling no pipes must work")?;
    assert_eq!(os.events(), [Event::Watched(Vec::new())]);
    Ok(())
}

#[test]
fn a_wait_cut_short_by_a_signal_is_a_turn_in_which_nothing_arrived() -> Checked {
    let os = ScriptedOs::default().polls([Err(ErrorKind::Interrupted.into())]);
    poll_pipes(&os, None, None, None).required_because("an interrupted wait is not a failure")?;
    Ok(())
}

#[test]
fn a_wait_that_fails_ends_the_child_and_refuses_the_run() -> Checked {
    let os = ScriptedOs::default().polls([Err(io::Error::other("the wait failed"))]);
    let error =
        capture(&os, &spec(), None).refused_because("a failed wait leaves nothing to read")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    assert_eq!(cause(&error)?, "the wait failed");
    assert!(ended(&os.events()));
    Ok(())
}

#[test]
fn a_relay_hands_both_streams_to_one_receiver_in_the_order_they_arrive() -> Checked {
    let relayed = TerminalCommand::relayed("fake-tool", &[]);
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"out")]))
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"err")]))
        .exits([Ok(Some(exited(0)))]);
    let mut output = RecordedOutput::new();

    // 中継では、どちらのstreamから届いたbyteかを分けない。読めた順がそのまま画面の順になる。
    let status = run_relay(&os, os.clock(), &mut (), relayed.spec(), None, &mut output)
        .required_because("a child that has exited ends the relay")?;

    assert!(status.success());
    assert_eq!(output.text(), "outerr");
    Ok(())
}

#[test]
fn pipes_that_cannot_be_made_nonblocking_end_the_child() -> Checked {
    for (end, id) in [
        (End::Stdout, ErrorId::ExternalCommandOutputUnreadable),
        (End::Stderr, ErrorId::ExternalCommandOutputUnreadable),
        (End::Stdin, ErrorId::ExternalCommandInputUnwritable),
    ] {
        let os = ScriptedOs::default().failing(
            Step::Nonblocking(end),
            io::Error::other("the flags could not be set"),
        );
        let spec = spec().with_input(b"hello".to_vec());

        let error = capture(&os, &spec, None).refused_because("a pipe that would block")?;

        assert_eq!(error.first_id(), Some(id), "{end:?}");
        assert_eq!(cause(&error)?, "the flags could not be set");
        assert!(ended(&os.events()), "{end:?}");
    }
    Ok(())
}

// --- stdinへ渡すbyte列 ---

#[test]
fn input_is_written_a_piece_at_a_time_and_closed_once_it_is_all_written() -> Checked {
    let writer = ScriptedWriter::answering([
        Ok(2),
        Err(ErrorKind::WouldBlock.into()),
        Err(ErrorKind::Interrupted.into()),
        Ok(3),
    ]);
    let written = writer.written();
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
    assert!(
        feed.waiting().is_none(),
        "the child sees the end of its input"
    );
    feed.feed().required()?;
    Ok(())
}

#[test]
fn a_child_that_stopped_reading_ends_the_input_without_an_error() -> Checked {
    let mut feed = InputFeed::new(
        ScriptedWriter::answering([Err(ErrorKind::BrokenPipe.into())]),
        b"hello",
    );
    // 結果は子の終了statusが決める。書けなかったことだけで実行を失敗にしない。
    feed.feed().required()?;
    assert!(
        feed.waiting().is_none(),
        "a child that stopped reading gets no more input"
    );
    feed.feed().required()?;

    // 何も受け付けない書き込み端も、次に書ける時まで待つだけである。
    let mut stalled = InputFeed::new(ScriptedWriter::answering([]), b"hello");
    stalled.feed().required()?;
    assert!(
        stalled.waiting().is_some(),
        "the input stays open until it is written"
    );
    Ok(())
}

#[test]
fn input_that_cannot_be_written_ends_the_child_and_keeps_what_the_os_said() -> Checked {
    let os = ScriptedOs::default().stdin(ScriptedWriter::answering([Err(io::Error::other(
        "the pipe could not be written",
    ))]));
    let spec = spec().with_input(b"hello".to_vec());

    let error = capture(&os, &spec, None)
        .refused_because("input that cannot be written refuses the run")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandInputUnwritable)
    );
    assert_eq!(cause(&error)?, "the pipe could not be written");
    assert!(ended(&os.events()), "the child does not outlive its input");
    Ok(())
}

#[test]
fn input_for_a_child_started_without_an_input_pipe_is_refused() -> Checked {
    let os = ScriptedOs::default().without(End::Stdin);
    let spec = spec().with_input(b"hello".to_vec());

    let error = capture(&os, &spec, None).refused_because("there is nowhere to write the input")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandInputUnwritable)
    );
    assert!(ended(&os.events()));
    Ok(())
}
