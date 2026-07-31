use crate::command::HostEnvironment;
use crate::config::ConfigLocation;

use crate::commands::status::global::host_commands::check_host_commands;
use crate::commands::status::global::platform::check_platform;
use crate::commands::status::global::sandboxes::check_docker_sandboxes;
use crate::commands::status::global::settings::{
    check_config, check_git_identity, check_registry, check_state_directory,
};

use super::GlobalStatus;

/// hostとglobal環境を診断する。何も変更しない。
pub fn diagnose(location: &ConfigLocation, host: &dyn HostEnvironment) -> GlobalStatus {
    let mut status = GlobalStatus {
        rows: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. global state directory、config、registry
    check_state_directory(location, &mut status);
    let config = check_config(location, &mut status);
    check_registry(location, &mut status);

    // 2. platform
    check_platform(host, &mut status);

    // 3-4. hostが直接実行するcommandと、Docker Client/Server疎通
    let present = check_host_commands(host, &mut status);

    // 5. 新規登録の既定となるGit identity
    check_git_identity(config.as_ref(), &mut status);

    // 5-9. Docker Sandboxes CLIとそのserviceの状態
    check_docker_sandboxes(host, present.contains(&"sbx"), &mut status);

    status
}
