use crate::boundary::host::protocol::{CliVersion, MINIMUM_CLI_VERSION, require_minimum_version};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::support::host_tools;

/// Docker Sandboxes CLIが、このbuildの求める最低version以上であることを確かめる。
///
/// 最低versionより古いCLIは出力の形が違い、後続の観測は原因を示さずに「出力を解釈
/// できない」として止まる。そのため、どの観測よりも先にversionだけを読む。versionを
/// 読めなければ、対応するversionだと推測せずに止める。
///
/// 同じ実行ファイルの確認が一度通っていれば、`host_tools`の記録により読まずに済ませ、
/// 工程も示さない。実行ファイルの観測は`sbx version`を読む前に済ませ、記録はその観測と
/// 対にする。読んだあとに観測すると、読んだあいだに差し替わった実行ファイルへ、読む前の
/// versionを結び付けうる。
pub fn require_supported_version(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    if !host.command_exists("sbx") {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", program = "sbx"),
            )
            .remediation(msg!("remediation-install-command", program = "sbx")),
        ));
    }
    let executable = host.inspect_executable("sbx");
    if host_tools::recorded_version(
        location,
        "sbx",
        executable.as_ref(),
        MINIMUM_CLI_VERSION,
        progress,
    )
    .is_some()
    {
        return Ok(());
    }
    progress.step(msg!("progress-inspect-sbx-version"));
    let spec = CommandSpec::capture("sbx", &["version"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::Probe);
    let output = host.run(&spec)?.require_success()?.stdout_text();
    let observed = CliVersion::extract_from_output(&output).ok_or_else(|| {
        Error::single(Diagnostic::new(
            ErrorId::SbxVersionUnparseable,
            msg!("error-sbx-version-unparseable", observed = output.trim()),
        ))
    })?;
    require_minimum_version(observed)?;
    host_tools::record(location, "sbx", executable.as_ref(), observed, progress);
    Ok(())
}
