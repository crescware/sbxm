use std::process::{Command, Stdio};

use super::{CommandSpec, OutputPolicy, apply_env};

/// program、argument、environment、作業directory、streamの向き先を決める。
pub(super) fn configure(spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.program);
    if spec.read_only
        && spec.program == "sbx"
        && spec.args.first().is_some_and(|arg| arg == "exec")
        && let Some(separator) = spec.args.iter().position(|arg| arg == "--")
    {
        command.args(&spec.args[..=separator]);
        command.args(["env", "GIT_OPTIONAL_LOCKS=0", "GIT_NO_LAZY_FETCH=1"]);
        command.args(&spec.args[separator + 1..]);
    } else {
        command.args(&spec.args);
    }
    if spec.read_only && spec.program == "git" {
        command.env("GIT_OPTIONAL_LOCKS", "0");
        command.env("GIT_NO_LAZY_FETCH", "1");
    }
    apply_env(
        &mut command,
        &spec.program,
        spec.env,
        spec.working_dir.as_deref(),
    );
    if let Some(directory) = &spec.working_dir {
        command.current_dir(directory);
    }
    match spec.output() {
        OutputPolicy::Capture | OutputPolicy::Relay => {
            // 渡すものが無ければ、stdinを待つ子にもすぐEOFを届ける。
            if spec.input().is_some() {
                command.stdin(Stdio::piped());
            } else {
                command.stdin(Stdio::null());
            }
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
        }
        OutputPolicy::HandOver => {
            // 利用者の入力をそのまま渡す。
            command.stdin(Stdio::inherit());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
        }
    }
    command
}
