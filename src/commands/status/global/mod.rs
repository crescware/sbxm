//! `sbxm status --global`。
//!
//! hostとglobal環境をread-onlyで診断する。login、setup、file更新、daemon起動・停止を
//! 行わない。問題がある場合は、利用者が直接実行する外部commandを表示する。
//!
//! 検査対象は、sbxm自身がhost上で直接使用する設定、platform、command、serviceに限定する。

mod external;
mod host_commands;
mod platform;
mod sandboxes;
mod service;
mod settings;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Msg};

use crate::support::{Row, StatusValue};

use host_commands::check_host_commands;
use platform::check_platform;
use sandboxes::check_docker_sandboxes;
use settings::{check_base_path, check_config};

/// 診断結果。
pub struct GlobalStatus {
    pub rows: Vec<Row>,
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<Msg>,
}
impl GlobalStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
/// hostとglobal環境を診断する。何も変更しない。
pub fn diagnose(location: &ConfigLocation, host: &dyn HostEnvironment) -> GlobalStatus {
    let mut status = GlobalStatus {
        rows: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. global configとbase path
    let config = check_config(location, &mut status);
    check_base_path(config.as_deref(), &mut status);

    // 2. platform
    check_platform(host, &mut status);

    // 3-4. hostが直接実行するcommandと、Docker Client/Server疎通
    let present = check_host_commands(host, &mut status);

    // 5-9. Docker Sandboxes CLIとそのserviceの状態
    check_docker_sandboxes(host, present.contains(&"sbx"), &mut status);

    status
}
fn push(status: &mut GlobalStatus, item: &'static str, value: StatusValue) {
    status.rows.push(Row {
        item,
        status: value,
    });
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
