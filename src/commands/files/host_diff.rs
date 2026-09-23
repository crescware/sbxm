use std::path::Path;

use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::paths;

/// hostのgitで、2つのfileの差分をunified形式で求める。
///
/// 外部diff、textconv、attributesは使わない。どれも利用者の設定でcommandを走らせうる。
/// 差分があることはgitの失敗ではないため、差分を書いた終了status`1`も受け付ける。gitは
/// fileを読めなかった場合も`1`で終わるため、何も書かなかった`1`は失敗として扱う。
pub fn host_diff(host: &dyn HostEnvironment, before: &Path, after: &Path) -> Result<String> {
    let before = paths::display(before);
    let after = paths::display(after);
    let spec = CommandSpec::capture(
        "git",
        &[
            "-c",
            "core.attributesFile=/dev/null",
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--",
            &before,
            &after,
        ],
    )
    .env(EnvPolicy::InheritWithoutSshAgent)
    .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?;
    if outcome.status.code() == Some(1) && !outcome.stdout.is_empty() {
        return Ok(outcome.stdout_text());
    }
    Ok(outcome.require_success()?.stdout_text())
}
