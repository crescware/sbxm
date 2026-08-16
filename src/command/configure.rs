use std::process::{Command, Stdio};

use super::{CommandSpec, EnvPolicy, OutputPolicy};

/// program、argument、environment、作業directory、streamの向き先を決める。
pub(super) fn configure(spec: &CommandSpec) -> Command {
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
            command.stdin(Stdio::null());
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
