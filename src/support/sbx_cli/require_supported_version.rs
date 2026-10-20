use crate::boundary::host::protocol::{CliVersion, require_minimum_version};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// Docker Sandboxes CLIが、このbuildの求める最低version以上であることを確かめる。
///
/// 最低versionより古いCLIは出力の形が違い、後続の観測は原因を示さずに「出力を解釈
/// できない」として止まる。そのため、どの観測よりも先にversionだけを読む。versionを
/// 読めなければ、対応するversionだと推測せずに止める。
pub fn require_supported_version(
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-inspect-sbx-version"));
    if !host.command_exists("sbx") {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", program = "sbx"),
            )
            .remediation(msg!("remediation-install-command", program = "sbx")),
        ));
    }
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
    require_minimum_version(observed)
}
