use std::process::{Command, Stdio};
use std::time::Duration;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{CommandOutcome, CommandSpec, EnvPolicy, OutputPolicy, spawn_reader, wait_with_limit};

pub(super) fn run_inner(spec: &CommandSpec, limit: Option<Duration>) -> Result<CommandOutcome> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    // defaultで現在processのenvironmentを継承する。
    if spec.env == EnvPolicy::InheritWithoutSshAgent {
        command.env_remove("SSH_AUTH_SOCK");
    }
    if let Some(directory) = &spec.working_dir {
        command.current_dir(directory);
    }
    match spec.output {
        OutputPolicy::Capture => {
            command.stdin(Stdio::null());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
        }
        OutputPolicy::Passthrough => {
            command.stdin(Stdio::null());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
        }
        OutputPolicy::Inherit => {
            // 利用者の入力をそのまま渡す。
            command.stdin(Stdio::inherit());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
        }
    }

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )
        } else {
            Error::new(
                ErrorId::ExternalCommandSpawnFailed,
                msg!(
                    "error-external-command-spawn-failed",
                    program = spec.program,
                    detail = error
                ),
            )
        }
    })?;

    // pipeが埋まって子processが止まらないよう、両streamを並行に読む。
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);

    let status = wait_with_limit(&mut child, spec, limit)?;

    let stdout = stdout_reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr_lossy = matches!(String::from_utf8_lossy(&stderr), std::borrow::Cow::Owned(_));

    Ok(CommandOutcome {
        program: spec.program.clone(),
        args: spec.args.clone(),
        working_dir: spec.working_dir.clone(),
        status,
        stdout,
        stderr,
        stderr_lossy,
    })
}
