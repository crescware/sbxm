use std::path::Path;

use crate::command::HostEnvironment;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths::{self};
use crate::support::docker;

use super::ephemeral_context;

/// project file、config、secretを含まない空のbuild contextでbuildする。
pub(super) fn build(
    host: &dyn HostEnvironment,
    name: &str,
    labels: &[(String, String)],
    dockerfile: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<Warning>> {
    let context = ephemeral_context()?;

    let mut args: Vec<String> = Vec::new();
    for (key, value) in labels {
        args.push("--label".to_string());
        args.push(format!("{key}={value}"));
    }
    args.push("--tag".to_string());
    args.push(name.to_string());
    args.push("--file".to_string());
    args.push(paths::display(dockerfile));
    args.push(paths::display(context.path()));

    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    progress.step(msg!("progress-building-image"));
    let result = docker::build(host, &borrowed, progress);

    // 成功・失敗にかかわらず、この実行が作った一時directoryを削除する。
    let leftover = paths::display(context.path());
    let mut warnings = Vec::new();
    if let Err(error) = context.close() {
        warnings.push(
            Warning::text(msg!("warning-build-context-left-behind"))
                .fact(Fact::path(&leftover))
                .fact(Fact::cause(&error.to_string())),
        );
    }
    result?;
    Ok(warnings)
}
