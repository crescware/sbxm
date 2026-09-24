use std::path::Path;
use std::process::Command;

use super::EnvPolicy;

/// gitが別のrepositoryへ移るときに消す変数（`git rev-parse --local-env-vars`）と、refの
/// 名前空間を変える`GIT_NAMESPACE`。どれも、呼び出し元が扱うrepositoryの場所や見え方を表す。
const REPOSITORY_LOCATION: [&str; 16] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

/// `policy`に従って、子processへ渡すenvironmentを変える。
///
/// defaultで現在processのenvironmentを継承する。`env_clear`は呼ばない。取り除くのは
/// `policy`が名指しする変数だけであり、`DOCKER_SANDBOXES_ROOT_SIZE`のような他の変数は
/// そのまま子processへ渡る。
pub(super) fn apply_env(command: &mut Command, policy: EnvPolicy, working_dir: Option<&Path>) {
    match policy {
        EnvPolicy::Inherit => {}
        EnvPolicy::InheritWithoutSshAgent => {
            command.env_remove("SSH_AUTH_SOCK");
        }
        EnvPolicy::HostRepository => {
            command.env_remove("SSH_AUTH_SOCK");
            for name in REPOSITORY_LOCATION {
                command.env_remove(name);
            }
            // 作業directoryがrepositoryでなければ、上のdirectoryのrepositoryを使わずに失敗させる。
            if let Some(parent) = working_dir.and_then(Path::parent) {
                command.env("GIT_CEILING_DIRECTORIES", parent);
            }
        }
    }
}
