use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{
    CommandOutcome, CommandSpec, EnvPolicy, OutputPolicy, collect_reader, isolates_process_group,
    spawn_reader, wait_with_limit,
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

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )
        } else {
            Error::single(
                Diagnostic::new(
                    ErrorId::ExternalCommandSpawnFailed,
                    msg!("error-external-command-spawn-failed"),
                )
                .fact(Fact::command(&spec.program))
                .fact(Fact::cause(&error.to_string())),
            )
        }
    })?;

    // pipeが埋まって子processが止まらないよう、両streamを並行に読む。
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);

    // 子processがどう終わっても、readerはこの関数の中で回収する。先に返ると、pipeを
    // 読んでいるthreadが実行の外側へ残る。
    let ended = wait_with_limit(&mut child, spec, limit);
    let stdout = collect_reader(stdout_reader, spec);
    let stderr = collect_reader(stderr_reader, spec);

    // 終わり方の異常を先に報告する。出力を読み切れていないのは、その結果でしかない。
    let status = ended?;
    let stdout = stdout?;
    let stderr = stderr?;
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
