use crate::boundary::host::Executable;
use crate::boundary::host::protocol::CliVersion;
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::msg;
use crate::paths;

use super::{Loaded, Unused, digest, load, reusable};

/// 記録により`program`のversion確認を省略できるなら、記録されたversionを返す。
///
/// 省略できるのは、観測した実行ファイルが再利用に適し、その識別情報が記録と一致し、記録の
/// digestが一致し、記録されたversionが`minimum`以上であるときだけである。どれか1つでも
/// 満たさなければ`None`を返し、呼び出し側は通常どおりversionを確認する。使えなかった理由は
/// traceにだけ残す。
pub fn recorded_version(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
    minimum: CliVersion,
    progress: &mut dyn ProgressSink,
) -> Option<CliVersion> {
    match lookup(location, program, executable, minimum) {
        Ok((observed, path)) => {
            progress.trace(msg!(
                "trace-host-tool-skipped",
                program = program,
                observed = observed,
                path = path
            ));
            Some(observed)
        }
        Err(reason) => {
            progress.trace(msg!(
                "trace-host-tool-checked",
                program = program,
                reason = reason.as_str()
            ));
            None
        }
    }
}

fn lookup(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
    minimum: CliVersion,
) -> Result<(CliVersion, String), Unused> {
    let executable = executable.ok_or(Unused::ExecutableUnobserved)?;
    reusable(program, executable).map_err(Unused::NotReusable)?;
    let records = match load(&location.host_tools_file()) {
        Loaded::Missing => return Err(Unused::RecordMissing),
        Loaded::Unusable => return Err(Unused::RecordUnusable),
        Loaded::Corrupt => return Err(Unused::RecordCorrupt),
        Loaded::Records(records) => records,
    };
    let recorded = records.get(program).ok_or(Unused::RecordMissing)?;
    if recorded.identity != executable.identity {
        return Err(Unused::ExecutableChanged);
    }
    if recorded.digest != digest(program, &recorded.identity, recorded.observed) {
        return Err(Unused::DigestMismatch);
    }
    if recorded.observed < minimum {
        return Err(Unused::VersionBelowMinimum);
    }
    Ok((recorded.observed, paths::display(&recorded.identity.path)))
}
