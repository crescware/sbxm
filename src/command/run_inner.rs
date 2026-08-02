use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{
    CommandOutcome, CommandSpec, EnvPolicy, OutputPolicy, SignalGuard, isolates_process_group,
    run_capture, spawn_failure, wait_with_limit,
};

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
    if isolates_process_group(spec) {
        // 打ち切るときに、このcommandが起動したprocessまで一度に終わらせられるようにする。
        command.process_group(0);
    }

    // SIGINTはsigactionが拒むsignalではなく、既に手当てされたsignalへ2つ目のactionを足す
    // ときはsyscallも呼ばない。それでも失敗を握り潰さない。Ctrl-Cを記録できないまま専用の
    // process groupへ子を置けば、利用者が中断したあとに子だけが残る。
    let signal = if isolates_process_group(spec) {
        Some(SignalGuard::new().map_err(|error| spawn_failure(spec, &error))?)
    } else {
        None
    };

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )
        } else {
            spawn_failure(spec, &error)
        }
    })?;

    if let Some(signal) = signal.as_ref() {
        let captured = run_capture(&mut child, spec, limit, signal);
        let (status, stdout, stderr) = captured?;
        let stderr_lossy = matches!(String::from_utf8_lossy(&stderr), std::borrow::Cow::Owned(_));

        return Ok(CommandOutcome {
            program: spec.program.clone(),
            args: spec.args.clone(),
            working_dir: spec.working_dir.clone(),
            status,
            stdout,
            stderr,
            stderr_lossy,
        });
    }

    let status = wait_with_limit(&mut child, spec, limit)?;
    let stdout = Vec::new();
    let stderr = Vec::new();
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
