use std::io::{self, ErrorKind};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::Duration;

use crate::diagnostics::ErrorId;
use crate::testing::command::{Event, ReadStep, ScriptedController, ScriptedOs, Step};
use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// promptを短く待つ、既定のcommand。
fn command(expected_prompt: &str) -> PtyConfirmedCommand {
    let mut command = PtyConfirmedCommand::new("fake-tool", &[], "the sandbox", expected_prompt);
    command.prompt_timeout = Duration::from_millis(60);
    command
}

/// `code`で自ら終わった子の終了status。
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

/// 別の場所で引き取られた子を尋ねたときの失敗。
fn no_child() -> io::Error {
    io::Error::from_raw_os_error(rustix::io::Errno::CHILD.raw_os_error())
}

/// 子を終わらせ、終了statusを引き取ったか。
fn ended(events: &[Event]) -> bool {
    events
        .windows(2)
        .any(|pair| pair == [Event::Ended, Event::WaitedExit])
}

/// promptとして`buffer`を示す端末側が、`command`の確認を通らないことを確かめる。
fn prompt_never_matches(buffer: &'static [u8], reason: &str) -> Checked {
    let controller = ScriptedController::new([ReadStep::Bytes(buffer)]);
    let written = controller.written();
    let os = ScriptedOs::default().controller(controller);

    let error =
        run_pty_confirmed(&os, os.clock(), &command("confirmation")).refused_because(reason)?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    assert!(written.borrow().is_empty(), "nothing was ever sent");
    Ok(())
}

#[test]
fn an_eof_from_the_controller_is_treated_as_a_missing_prompt() -> Checked {
    let os = ScriptedOs::default().controller(ScriptedController::new([ReadStep::Eof]));
    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("an EOF does not confirm the command")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    // 60msのprompt timeoutは、20msずつ進む3回の待ちで過ぎる。
    assert_eq!(os.clock().slept.borrow().len(), 3);
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn an_unreadable_controller_is_treated_as_a_missing_prompt() -> Checked {
    let os = ScriptedOs::default().controller(ScriptedController::new([ReadStep::Failed]));
    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("an unreadable controller does not confirm the command")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_answer_write_failure_is_reported_as_not_confirmed() -> Checked {
    let prompt = "confirmation";
    let controller = ScriptedController::new([ReadStep::Bytes(prompt.as_bytes())])
        .answering_writes([Err(io::Error::other("the write failed"))]);
    let os = ScriptedOs::default().controller(controller);

    let error = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .refused_because("a failed answer write is not confirmation")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_answer_flush_failure_is_reported_as_not_confirmed() -> Checked {
    let prompt = "confirmation";
    let controller = ScriptedController::new([ReadStep::Bytes(prompt.as_bytes())])
        .failing_flush(io::Error::other("the flush failed"));
    let os = ScriptedOs::default().controller(controller);

    let error = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .refused_because("a failed flush is not confirmation")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn a_wait_failure_is_reported_as_a_spawn_failure() -> Checked {
    let os = ScriptedOs::default().exits([Err(no_child())]);
    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("a child that cannot be waited for is a spawn failure")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    Ok(())
}

#[test]
fn draining_an_empty_controller_is_complete() -> Checked {
    for step in [ReadStep::Eof, ReadStep::WouldBlock, ReadStep::Gone] {
        let mut controller = ScriptedController::new([step]);
        let mut buffer = Vec::new();
        drain_after_exit(&mut controller, &mut buffer, &command("confirmation"))
            .required_because("an empty controller is drained")?;
        assert!(buffer.is_empty());
    }
    Ok(())
}

#[test]
fn draining_an_unreadable_controller_is_reported() -> Checked {
    let mut controller = ScriptedController::new([ReadStep::Failed]);
    let mut buffer = Vec::new();
    let error = drain_after_exit(&mut controller, &mut buffer, &command("confirmation"))
        .refused_because("an unreadable controller cannot be drained")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    Ok(())
}

#[test]
fn a_matched_prompt_is_answered_exactly_once() -> Checked {
    let prompt = "confirmation";
    // promptは2度示されるが、答えた後の読みは`after_answer`へ移るため、2度目は誰も見ない。
    let controller = ScriptedController::new([
        ReadStep::Bytes(prompt.as_bytes()),
        ReadStep::Bytes(prompt.as_bytes()),
    ])
    .after_answer([ReadStep::Bytes(b"removed")]);
    let written = controller.written();
    let os = ScriptedOs::default()
        .controller(controller)
        .exits([Ok(Some(exited(0)))]);

    let outcome = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .required_because("the expected prompt is answered")?;

    assert!(outcome.success());
    assert!(String::from_utf8_lossy(&outcome.stderr).contains("removed"));
    assert_eq!(written.borrow().as_slice(), b"y\n");
    Ok(())
}

#[test]
fn a_runtime_refusal_after_the_prompt_is_answered_is_still_reported() -> Checked {
    let prompt = "confirmation";
    let controller = ScriptedController::new([ReadStep::Bytes(prompt.as_bytes())])
        .after_answer([ReadStep::Bytes(b"is in use")]);
    let os = ScriptedOs::default()
        .controller(controller)
        .exits([Ok(Some(exited(1)))]);

    let outcome = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .required_because("a refused removal is still a completed exchange")?;

    assert!(!outcome.success());
    assert!(String::from_utf8_lossy(&outcome.stderr).contains("is in use"));
    Ok(())
}

#[test]
fn a_runtime_refusal_past_one_read_chunk_is_still_reported_in_full() -> Checked {
    let prompt = "confirmation";
    // 4096byteの読み取りchunkを超える出力の末尾に拒否理由を置く。processが終わった後の
    // 読み取りを1回で打ち切ると、この末尾が欠落しうる。
    let controller = ScriptedController::new([ReadStep::Bytes(prompt.as_bytes())]).after_answer([
        ReadStep::Owned(vec![b'a'; 5000]),
        ReadStep::Bytes(b"is in use"),
        ReadStep::WouldBlock,
    ]);
    let os = ScriptedOs::default()
        .controller(controller)
        .exits([Ok(Some(exited(1)))]);

    let outcome = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .required_because("a refused removal is still a completed exchange")?;

    assert!(!outcome.success());
    assert!(
        outcome.stderr.len() > 4096,
        "the padding before the refusal survived: {} bytes",
        outcome.stderr.len()
    );
    assert!(String::from_utf8_lossy(&outcome.stderr).contains("is in use"));
    Ok(())
}

#[test]
fn a_different_prompt_is_never_answered() -> Checked {
    prompt_never_matches(
        b"Delete this sandbox? (y/N): ",
        "a prompt that does not match the expected text is never answered",
    )
}

#[test]
fn an_invalid_byte_right_after_the_expected_prompt_is_never_answered() -> Checked {
    // 期待文字列そのものの直後に、単独では有効なUTF-8にならないbyteを続ける。prefixだけを
    // 見て一致とみなすと、この続きを確かめないまま答えてしまう。
    prompt_never_matches(
        b"confirmation\x80",
        "an expected prefix followed by an invalid byte is never answered",
    )
}

#[test]
fn a_prompt_that_drifts_past_the_expected_text_on_the_same_line_is_never_answered() -> Checked {
    // 期待文字列を先頭に含むが、そこで終わらず追記が続く。
    prompt_never_matches(
        b"confirmation are you sure? ",
        "text appended after the expected prompt is never answered",
    )
}

#[test]
fn an_additional_question_after_the_expected_prompt_is_never_answered() -> Checked {
    // 期待文字列を含みはするが、観測済みの全体はそれで終わっていない。
    prompt_never_matches(
        b"confirmation Type CONFIRM to continue: ",
        "a second question after the expected prompt is never answered",
    )
}

#[test]
fn two_ascii_spaces_after_the_prompt_are_not_accepted() {
    assert!(!prompt_is_ready(b"confirmation  ", "confirmation"));
}

#[test]
fn a_tab_after_the_prompt_is_not_accepted() {
    assert!(!prompt_is_ready(b"confirmation\t", "confirmation"));
}

#[test]
fn a_newline_after_the_prompt_is_not_accepted() -> Checked {
    assert!(!prompt_is_ready(b"confirmation\n", "confirmation"));
    // driveを通しても確かめる: 改行1個も答えの引き金にしない。
    prompt_never_matches(
        b"confirmation\n",
        "a newline is not the exact prompt contract",
    )
}

#[test]
fn a_process_that_ends_before_the_prompt_appears_is_not_confirmed() -> Checked {
    let os = ScriptedOs::default().exits([Ok(Some(exited(1)))]);
    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("nothing was sent, so the exit code alone proves nothing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn invalid_utf8_never_satisfies_the_expected_prompt() -> Checked {
    // 0x80は単独では有効なUTF-8にならない。lossy変換で読めてしまうと誤って一致しかねない。
    prompt_never_matches(
        b"\x80\x80\x80\x80",
        "bytes that never decode to the expected text are never answered",
    )
}

#[test]
fn a_program_that_cannot_be_found_is_reported_without_opening_a_pty_forever() -> Checked {
    let os = ScriptedOs::default().failing(Step::Start, io::Error::from(ErrorKind::NotFound));

    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("a missing program is a spawn failure, not a confirmation failure")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    let events = os.events();
    assert!(events.contains(&Event::Pty(Step::OpenController)));
    assert!(events.contains(&Event::Pty(Step::SetSize)));
    assert!(!events.contains(&Event::Ended), "{events:?}");
    Ok(())
}

#[test]
fn a_timeout_diagnostic_names_the_external_command() {
    let mut command = PtyConfirmedCommand::new(
        "sbx",
        &[],
        "the sandbox",
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    );
    command.prompt_timeout = Duration::from_millis(500);

    let error = timed_out(&command);

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
}

#[test]
fn the_prompt_deadline_ends_the_child_before_it_is_reported() -> Checked {
    let os = ScriptedOs::default();
    let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
        .refused_because("the prompt never appears")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn the_overall_deadline_ends_the_child_after_it_is_answered() -> Checked {
    let prompt = "confirmation";
    let os = ScriptedOs::default().controller(ScriptedController::new([ReadStep::Bytes(
        prompt.as_bytes(),
    )]));

    let error = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .refused_because("the process never exits after being answered")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn each_pty_step_that_fails_is_a_spawn_failure_that_opens_nothing_else() -> Checked {
    let steps = [
        Step::OpenController,
        Step::CloseOnExec,
        Step::Grant,
        Step::UnlockTerminal,
        Step::TerminalName,
        Step::OpenTerminal,
        Step::Settings,
        Step::ApplySettings,
        Step::SetSize,
        Step::ControllerNonblocking,
        Step::TerminalStdio(1),
        Step::TerminalStdio(2),
        Step::TerminalStdio(3),
    ];
    for step in steps {
        let os = ScriptedOs::default().failing(step, io::Error::other("the pty step failed"));
        let error = run_pty_confirmed(&os, os.clock(), &command("confirmation"))
            .refused_because("a failed pty step is a spawn failure")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandSpawnFailed),
            "{step:?}"
        );
        assert!(
            !os.events()
                .iter()
                .any(|event| matches!(event, Event::Started { .. })),
            "{step:?}: {:?}",
            os.events()
        );
    }
    Ok(())
}

#[test]
fn the_pty_is_opened_in_the_documented_order() {
    let os = ScriptedOs::default();
    let _ = run_pty_confirmed(&os, os.clock(), &command("confirmation"));

    let events = os.events();
    assert_eq!(
        &events[..11],
        &[
            Event::Pty(Step::OpenController),
            Event::Pty(Step::CloseOnExec),
            Event::Pty(Step::Grant),
            Event::Pty(Step::UnlockTerminal),
            Event::Pty(Step::TerminalName),
            Event::Pty(Step::OpenTerminal),
            Event::Pty(Step::Settings),
            Event::MadeRaw,
            Event::Pty(Step::ApplySettings),
            Event::Pty(Step::SetSize),
            Event::Pty(Step::ControllerNonblocking),
        ][..],
        "{events:?}"
    );
    assert_eq!(
        &events[11..14],
        &[
            Event::Pty(Step::TerminalStdio(1)),
            Event::Pty(Step::TerminalStdio(2)),
            Event::Pty(Step::TerminalStdio(3)),
        ][..],
        "{events:?}"
    );
    assert!(matches!(events[14], Event::Started { .. }), "{events:?}");
    assert!(
        !events.contains(&Event::OwnGroup),
        "a PTY run does not move to its own process group"
    );
}

#[test]
fn an_answered_exchange_whose_last_output_cannot_be_read_is_reported() -> Checked {
    let prompt = "confirmation";
    let controller = ScriptedController::new([ReadStep::Bytes(prompt.as_bytes())])
        .after_answer([ReadStep::Failed]);
    let os = ScriptedOs::default()
        .controller(controller)
        .exits([Ok(Some(exited(0)))]);

    let error = run_pty_confirmed(&os, os.clock(), &command(prompt))
        .refused_because("the output after the exit is not known")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    Ok(())
}
