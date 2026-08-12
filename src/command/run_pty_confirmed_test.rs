use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// script1本を実行可能fileとして置く。
fn fake_script(dir: &Path, body: &str) -> Checked<PathBuf> {
    let path = dir.join("fake-sbx");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).required_because("write the fake script")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .required_because("make it executable")?;
    Ok(path)
}

/// promptを短いtimeoutで待つ以外は既定のcommand。
fn command(program: &Path, expected_prompt: &str) -> PtyConfirmedCommand {
    let mut command = PtyConfirmedCommand::new(
        program.to_str().unwrap_or_default(),
        &[],
        "the sandbox",
        expected_prompt,
    );
    command.prompt_timeout = Duration::from_millis(500);
    command
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
        &command(&script, "Remove sandbox 'x'? This cannot be undone. (y/N):")
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

    let outcome = run_pty_confirmed(&command(
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
    let error = run_pty_confirmed(&command(
        Path::new("/does/not/exist/sbx"),
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ))
    .refused_because("a missing program is a spawn failure, not a confirmation failure")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}

#[test]
fn a_timeout_diagnostic_names_the_external_command() {
    let error = timed_out(&command(
        Path::new("sbx"),
        "Remove sandbox 'x'? This cannot be undone. (y/N):",
    ));

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
}
