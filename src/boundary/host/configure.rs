use std::fs::File;
use std::process::{Command, Stdio};

use crate::diagnostics::Result;
use crate::paths;

use super::{CommandSpec, EnvPolicy, OutputPolicy, unwritable};

/// program、argument、environment、作業directory、streamの向き先を決める。
///
/// stdinへつなぐfileを開けなければ、子を起動せずに失敗する。
pub(super) fn configure(spec: &CommandSpec) -> Result<Command> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    // defaultで現在processのenvironmentを継承する。`env_clear`や`envs`は呼ばない。
    // `InheritWithoutSshAgent`は`SSH_AUTH_SOCK`だけを取り除き、それ以外の変数
    // （`DOCKER_SANDBOXES_ROOT_SIZE`を含む）はそのまま子processへ渡る。
    if spec.env == EnvPolicy::InheritWithoutSshAgent {
        command.env_remove("SSH_AUTH_SOCK");
    }
    if let Some(directory) = &spec.working_dir {
        command.current_dir(directory);
    }
    match spec.output() {
        OutputPolicy::Capture | OutputPolicy::Relay => {
            // 渡すbyte列が無ければ、stdinを待つ子にもすぐEOFを届ける。
            if spec.input.is_some() {
                command.stdin(Stdio::piped());
            } else if let Some(path) = &spec.input_file {
                let file = File::open(path).map_err(|error| {
                    unwritable(spec, &format!("{}: {error}", paths::display(path)))
                })?;
                command.stdin(Stdio::from(file));
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
