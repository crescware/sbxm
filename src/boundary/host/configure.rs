use std::fs::File;
use std::process::{Command, Stdio};

use crate::diagnostics::Result;

use super::{CommandSpec, OutputPolicy, apply_env, input_unavailable};

/// program、argument、environment、作業directory、streamの向き先を決める。
///
/// stdinへつなぐfileを開けなければ、子を起動せずに失敗する。
pub(super) fn configure(spec: &CommandSpec) -> Result<Command> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    apply_env(&mut command, spec.env, spec.working_dir.as_deref());
    if let Some(directory) = &spec.working_dir {
        command.current_dir(directory);
    }
    match spec.output() {
        OutputPolicy::Capture | OutputPolicy::Relay => {
            // 渡すものが無ければ、stdinを待つ子にもすぐEOFを届ける。
            if let Some(path) = spec.input_file() {
                let file = File::open(path)
                    .map_err(|error| input_unavailable(spec, path, &error.to_string()))?;
                command.stdin(Stdio::from(file));
            } else if spec.input().is_some() {
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
    Ok(command)
}
