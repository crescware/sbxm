use std::path::Path;

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths::{self};

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
    // buildの進捗はdockerが出したまま転送する。
    progress.step(msg!("progress-building-image"));
    let spec = CommandSpec::passthrough("docker", &[&["build"], borrowed.as_slice()].concat())
        .timeout(TimeoutClass::ImageBuild);
    let result = host
        .run(&spec)
        .and_then(crate::command::CommandOutcome::require_success);

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
