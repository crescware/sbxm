use std::collections::BTreeMap;

use crate::boundary::host::Executable;
use crate::boundary::host::protocol::CliVersion;
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::msg;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope};

use super::{Loaded, RecordedTool, Unsaved, load, render, reusable};

/// 最低versionを満たすと確認できた`program`の実行ファイルを記録する。
///
/// 確認が通ったあとにだけ呼ぶ。記録できなくても確認は通っているので、commandは続く。
/// 記録しなかった理由はtraceにだけ残す。
pub fn record(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
    observed: CliVersion,
    progress: &mut dyn ProgressSink,
) {
    if let Err(reason) = save(location, program, executable, observed) {
        progress.trace(msg!(
            "trace-host-tool-unsaved",
            program = program,
            reason = reason.as_str()
        ));
    }
}

fn save(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
    observed: CliVersion,
) -> Result<(), Unsaved> {
    let executable = executable.ok_or(Unsaved::ExecutableUnobserved)?;
    reusable(program, executable).map_err(Unsaved::NotReusable)?;
    // `~/.sbxm`は案件の登録や設定の保存が作る。記録のためだけには作らない。
    if !paths::directory_exists(&location.dir(), PathScope::ConfigDir).unwrap_or(false) {
        return Err(Unsaved::StateDirectoryMissing);
    }
    let path = location.host_tools_file();
    let (mut records, exists) = match load(&path) {
        Loaded::Missing => (BTreeMap::new(), false),
        Loaded::Corrupt => (BTreeMap::new(), true),
        Loaded::Unusable => return Err(Unsaved::RecordUnusable),
        Loaded::Records(records) => (records, true),
    };
    records.insert(
        program.to_string(),
        RecordedTool::new(program, executable.identity.clone(), observed),
    );
    let text = render(&records).map_err(|_| Unsaved::WriteFailed)?;
    let written = if exists {
        paths::atomic_replace(&path, &text, PRIVATE_FILE_MODE)
    } else {
        paths::atomic_create(&path, &text, PRIVATE_FILE_MODE)
    };
    written.map_err(|_| Unsaved::WriteFailed)
}
