use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::boundary::host::EnvPolicy;
use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// script1本を置く。
///
/// scriptは`sh <path>`で起動し、書いたfileそのものをexecしない。書いた直後のfileをexecすると、
/// 別threadのtestがforkした子が書き込み端を持ったままの間は`ETXTBSY`で起動できない。
fn fake_script(dir: &Path, body: &str) -> Checked<String> {
    let path = dir.join("fake-sbx");
    fs::write(&path, format!("{body}\n")).required_because("write the fake script")?;
    Ok(path.to_str().required()?.to_string())
}

/// promptを短いtimeoutで待つ以外は既定のcommand。
fn command(script: &str, expected_prompt: &str) -> PtyConfirmedCommand {
    command_with_prompt_timeout("sh", &[script], expected_prompt, Duration::from_millis(500))
}

/// coverage実行時のmacOSでも、shellが起動してpromptを出す時間を確保する。
fn command_with_prompt_timeout(
    program: &str,
    args: &[&str],
    expected_prompt: &str,
    prompt_timeout: Duration,
) -> PtyConfirmedCommand {
    let mut command = PtyConfirmedCommand::new(program, args, "the sandbox", expected_prompt);
    command.prompt_timeout = prompt_timeout;
    command
}

fn command_for_interactive_exchange(script: &str, expected_prompt: &str) -> PtyConfirmedCommand {
    // 正常系は、coverage buildでmacOSのshell起動が遅くなっても既定のprompt timeoutまで
    // 待つ。異常系の短いtimeoutは、promptが現れないことを確認するtestだけで使う。
    command_with_prompt_timeout("sh", &[script], expected_prompt, Duration::from_secs(20))
}

/// promptを待つloopだけを試すため、十分長く生きる直接の子processを用意する。
fn sleeping_child() -> Checked<std::process::Child> {
    std::process::Command::new("sleep")
        .arg("30")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .required_because("a child to keep the prompt loop alive")
}

fn short_prompt_command() -> PtyConfirmedCommand {
    command_with_prompt_timeout("sleep", &[], "confirmation", Duration::from_millis(1))
}

#[test]
fn an_eof_from_the_controller_is_treated_as_a_missing_prompt() -> Checked {
    let mut child = sleeping_child()?;
    let error = drive(
        &mut child,
        fs::File::open("/dev/null").required_because("an empty controller")?,
        &short_prompt_command(),
    )
    .refused_because("an EOF does not confirm the command")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_unreadable_controller_is_treated_as_a_missing_prompt() -> Checked {
    let mut child = sleeping_child()?;
    let controller = fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .required_because("an unreadable controller")?;
    let error = drive(&mut child, controller, &short_prompt_command())
        .refused_because("an unreadable controller does not confirm the command")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_answer_write_failure_is_reported_as_not_confirmed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    fs::write(dir.path().join("prompt"), b"confirmation")
        .required_because("a controller that contains the prompt")?;
    let controller =
        fs::File::open(dir.path().join("prompt")).required_because("a read-only controller")?;
    let mut child = sleeping_child()?;
    let error = drive(&mut child, controller, &short_prompt_command())
        .refused_because("a failed answer write is not confirmation")?;
    terminate_child(&RealHost, &mut child);

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn a_wait_failure_is_reported_as_a_spawn_failure() -> Checked {
    let mut child = sleeping_child()?;
    child
        .kill()
        .required_because("stop the child before reaping it outside its handle")?;
    rustix::process::waitpid(
        Some(rustix::process::Pid::from_child(&child)),
        rustix::process::WaitOptions::empty(),
    )
    .required_because("reap the child outside its handle")?;

    let error = drive(
        &mut child,
        fs::File::open("/dev/null").required_because("an empty controller")?,
        &short_prompt_command(),
    )
    .refused_because("a child that cannot be waited for is a spawn failure")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    Ok(())
}

#[test]
fn draining_an_empty_controller_is_complete() -> Checked {
    let mut buffer = Vec::new();
    drain_after_exit(
        &mut fs::File::open("/dev/null").required_because("an empty controller")?,
        &mut buffer,
        &short_prompt_command(),
    )
    .required_because("an empty controller is drained")?;
    assert!(buffer.is_empty());
    Ok(())
}

#[test]
fn draining_an_unreadable_controller_is_reported() -> Checked {
    let mut controller = fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .required_because("an unreadable controller")?;
    let mut buffer = Vec::new();
    let error = drain_after_exit(&mut controller, &mut buffer, &short_prompt_command())
        .refused_because("an unreadable controller cannot be drained")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    Ok(())
}

#[test]
fn a_matched_prompt_is_answered_exactly_once() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): \"\n\
         read reply\n\
         if [ \"$reply\" != \"y\" ]; then exit 9; fi\n\
         printf 'Deleting sandbox x...\\n'\n\
         printf \"Sandbox 'x' removed\\n\"\n",
    )?;

    let outcome = run_pty_confirmed(
        &command_for_interactive_exchange(
            &script,
            "Remove sandbox 'x'? This cannot be undone. (y/N):",
        )
        .env(EnvPolicy::InheritWithoutSshAgent),
    )
    .required_because("the expected prompt is answered")?;
    assert!(outcome.success(), "{}", outcome.stdout_text());
    assert!(
        String::from_utf8_lossy(&outcome.stderr).contains("removed"),
        "{}",
        String::from_utf8_lossy(&outcome.stderr)
    );
    Ok(())
}

#[test]
fn a_runtime_refusal_after_the_prompt_is_answered_is_still_reported() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): \"\n\
         read reply\n\
         printf 'Deleting sandbox x...\\n'\n\
         printf \"Error: sandbox 'x' is in use; close it or re-run with --force\\n\" >&2\n\
         exit 1\n",
    )?;

    let outcome = run_pty_confirmed(&command_for_interactive_exchange(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .required_because("a refused removal is still a completed exchange")?;
    assert!(!outcome.success());
    assert!(
        String::from_utf8_lossy(&outcome.stderr).contains("is in use"),
        "the runtime's own reason is preserved: {}",
        String::from_utf8_lossy(&outcome.stderr)
    );
    Ok(())
}

#[test]
fn a_runtime_refusal_past_one_read_chunk_is_still_reported_in_full() -> Checked {
    let dir = tempfile::tempdir().required()?;
    // 4096byteの読み取りchunkを超える出力の末尾に拒否理由を置く。processが終わった後の
    // 読み取りを1回で打ち切ると、この末尾が欠落しうる。
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): \"\n\
         read reply\n\
         awk 'BEGIN { for (i = 0; i < 5000; i++) printf \"a\" }'\n\
         printf '\\n'\n\
         printf \"Error: sandbox 'x' is in use; close it or re-run with --force\\n\" >&2\n\
         exit 1\n",
    )?;

    let outcome = run_pty_confirmed(&command_for_interactive_exchange(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .required_because("a refused removal is still a completed exchange")?;
    assert!(!outcome.success());
    assert!(
        outcome.stderr.len() > 4096,
        "the padding before the refusal survived: {} bytes",
        outcome.stderr.len()
    );
    assert!(
        String::from_utf8_lossy(&outcome.stderr).contains("is in use"),
        "the runtime's own reason is preserved past the first read chunk: {}",
        String::from_utf8_lossy(&outcome.stderr)
    );
    Ok(())
}

#[test]
fn a_different_prompt_is_never_answered() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let script = fake_script(
        dir.path(),
        "printf 'Delete this sandbox? (y/N): '\n\
         read reply\n\
         if [ \"$reply\" = 'y' ]; then printf 'removed\\n'; fi\n\
         sleep 5\n",
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("a prompt that does not match the expected text is never answered")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_invalid_byte_right_after_the_expected_prompt_is_never_answered() -> Checked {
    let dir = tempfile::tempdir().required()?;
    // 期待文字列そのものの直後に、単独では有効なUTF-8にならないbyteを続ける。prefixだけを
    // 見て一致とみなすと、この続きを確かめないまま答えてしまう。
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): \\200\"\n\
         sleep 5\n",
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("an expected prefix followed by an invalid byte is never answered")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn a_prompt_that_drifts_past_the_expected_text_on_the_same_line_is_never_answered() -> Checked {
    let dir = tempfile::tempdir().required()?;
    // 期待文字列を先頭に含むが、そこで終わらず追記が続く。containsだけで確かめると、
    // 続きを読まずに答えてしまう。
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): are you sure? \"\n\
         sleep 5\n",
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("text appended after the expected prompt is never answered")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn an_additional_question_after_the_expected_prompt_is_never_answered() -> Checked {
    let dir = tempfile::tempdir().required()?;
    // protocolが変わり、期待するpromptの後に別の質問が続く場合。期待文字列を含みはするが、
    // 観測済みの全体はそれで終わっていない。
    let script = fake_script(
        dir.path(),
        "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): \"\n\
         printf 'Type CONFIRM to continue: '\n\
         sleep 5\n",
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("a second question after the expected prompt is never answered")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

fn prompt_suffix_is_not_accepted(suffix: &str, reason: &str) -> Checked {
    let dir = tempfile::tempdir().required()?;
    let script = fake_script(
        dir.path(),
        &format!(
            "printf \"Remove sandbox 'x'? This cannot be undone. (y/N): {suffix}\"\n\
             sleep 5\n"
        ),
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because(reason)?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn two_ascii_spaces_after_the_prompt_are_not_accepted() -> Checked {
    prompt_suffix_is_not_accepted(
        "  ",
        "two trailing spaces are not the exact prompt contract",
    )
}

#[test]
fn a_tab_after_the_prompt_is_not_accepted() -> Checked {
    prompt_suffix_is_not_accepted("\t", "a tab is not the exact prompt contract")
}

#[test]
fn a_newline_after_the_prompt_is_not_accepted() -> Checked {
    prompt_suffix_is_not_accepted("\n", "a newline is not the exact prompt contract")
}

#[test]
fn a_process_that_ends_before_the_prompt_appears_is_not_confirmed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let script = fake_script(dir.path(), "exit 1\n")?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("nothing was sent, so the exit code alone proves nothing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn invalid_utf8_never_satisfies_the_expected_prompt() -> Checked {
    let dir = tempfile::tempdir().required()?;
    // 0x80は単独では有効なUTF-8にならない。lossy変換で読めてしまうと誤って一致しかねない。
    let script = fake_script(
        dir.path(),
        "printf '\\200\\200\\200\\200'\n\
         sleep 5\n",
    )?;

    let error = run_pty_confirmed(&command(
        &script,
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("bytes that never decode to the expected text are never answered")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotConfirmed));
    Ok(())
}

#[test]
fn a_program_that_cannot_be_found_is_reported_without_opening_a_pty_forever() -> Checked {
    let error = run_pty_confirmed(&command_with_prompt_timeout(
        "/does/not/exist/sbx",
        &[],
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
        Duration::from_millis(500),
    ))
    .refused_because("a missing program is a spawn failure, not a confirmation failure")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}

#[test]
fn a_timeout_diagnostic_names_the_external_command() {
    let error = timed_out(&command_with_prompt_timeout(
        "sbx",
        &[],
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
        Duration::from_millis(500),
    ));

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
}
