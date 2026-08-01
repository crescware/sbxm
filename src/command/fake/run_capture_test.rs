use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

use crate::command::CommandSpec;
use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};

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

fn signal() -> Checked<SignalGuard> {
    SignalGuard::new().required_because("SIGINT registration must succeed")
}

#[test]
fn a_missing_stdout_pipe_is_reported() -> Checked {
    let mut child = child(Stdio::null(), Stdio::piped())?;
    let signal = signal()?;
    let error = run_capture(
        &mut child,
        &CommandSpec::probe("sh", &["-c", "sleep 30"]),
        None,
        &signal,
    )
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
    let error = run_capture(
        &mut child,
        &CommandSpec::probe("sh", &["-c", "sleep 30"]),
        None,
        &signal,
    )
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

    let error = run_capture(
        &mut child,
        &CommandSpec::probe("sh", &["-c", "sleep 30"]),
        None,
        &signal,
    )
    .refused_because("an interrupted capture must be canceled")?;

    assert!(matches!(error, crate::diagnostics::Error::Canceled));
    Ok(())
}

#[test]
fn a_child_reaped_before_capture_is_reported_as_a_wait_failure() -> Checked {
    let mut command = Command::new("sh");
    command.args(["-c", "exit 0"]);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .required_because("the test child must start")?;
    rustix::process::waitpid(
        Some(rustix::process::Pid::from_child(&child)),
        rustix::process::WaitOptions::empty(),
    )
    .required_because("the test child must be reaped")?;
    let signal = signal()?;

    let error = run_capture(
        &mut child,
        &CommandSpec::probe("sh", &["-c", "exit 0"]),
        None,
        &signal,
    )
    .refused_because("a child that cannot be waited must be refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    Ok(())
}

#[test]
fn drain_pipe_handles_empty_and_read_errors() {
    let mut collected = Vec::new();
    let mut absent: Option<ScriptedPipe> = None;
    assert!(drain_pipe(&mut absent, &mut collected).is_ok());

    let mut pipe = Some(ScriptedPipe::new([
        ReadStep::Interrupted,
        ReadStep::Bytes(b"out"),
        ReadStep::WouldBlock,
    ]));
    assert!(drain_pipe(&mut pipe, &mut collected).is_ok());
    assert!(
        pipe.is_some(),
        "an interrupted read leaves the pipe available"
    );
    assert_eq!(collected, b"");

    assert!(drain_pipe(&mut pipe, &mut collected).is_ok());
    assert_eq!(collected, b"out");

    let mut failed = Some(ScriptedPipe::new([ReadStep::Failed]));
    assert!(drain_pipe(&mut failed, &mut collected).is_err());
}

#[test]
fn poll_pipes_accepts_missing_streams() -> Checked {
    assert_eq!(
        poll_pipes(None, None).required_because("polling no pipes must work")?,
        (false, false)
    );
    Ok(())
}

enum ReadStep {
    Bytes(&'static [u8]),
    Interrupted,
    WouldBlock,
    Failed,
}

struct ScriptedPipe {
    steps: VecDeque<ReadStep>,
}

impl ScriptedPipe {
    fn new(steps: impl IntoIterator<Item = ReadStep>) -> ScriptedPipe {
        ScriptedPipe {
            steps: steps.into_iter().collect(),
        }
    }
}

impl Read for ScriptedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.steps.pop_front() {
            Some(ReadStep::Bytes(bytes)) => {
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Some(ReadStep::Interrupted) => Err(io::ErrorKind::Interrupted.into()),
            Some(ReadStep::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Some(ReadStep::Failed) => Err(io::Error::other("the pipe could not be read")),
            None => Ok(0),
        }
    }
}
