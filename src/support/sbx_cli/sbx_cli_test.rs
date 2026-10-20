use crate::boundary::host::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::diagnostics::ErrorId;
use crate::testing::global_status::FakeHost;
use crate::testing::host::{FakeSbx, supported_sbx_version};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

#[test]
fn the_minimum_version_is_read_with_a_captured_probe_and_shown_as_a_step() -> Checked {
    let host = FakeSbx::listing("[]");
    let mut progress = RecordedOutput::new();
    require_supported_version(&host, &mut progress).required()?;

    let spec = host.spec("version")?;
    assert_eq!(spec.program, "sbx");
    assert_eq!(spec.args, ["version"]);
    assert_eq!(spec.timeout, TimeoutClass::Probe);
    assert_eq!(spec.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(spec.output(), OutputPolicy::Capture);
    assert_eq!(host.calls().len(), 1);
    assert_eq!(progress.steps.len(), 1);
    assert_eq!(progress.steps[0].id, "progress-inspect-sbx-version");
    Ok(())
}

#[test]
fn an_older_cli_is_refused_with_the_version_to_update_to() -> Checked {
    // v0.47.0が実際に返す形。
    let host = FakeSbx::listing("[]").answering(
        "version",
        0,
        "sbx version: v0.47.0 0411f50ee4700fe7bd37e6e7e3aced563e850ca9\n",
    );
    let error =
        require_supported_version(&host, &mut RecordedOutput::new()).refused_because("0.47.0")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::SbxVersionBelowMinimum);
    let remediation = diagnostic.remediation.as_ref().required()?;
    assert_eq!(remediation.explanation[0].id, "remediation-sbx-update");
    Ok(())
}

#[test]
fn a_version_that_cannot_be_read_is_not_taken_for_a_supported_one() -> Checked {
    for output in ["", "unreleased build\n"] {
        let host = FakeSbx::listing("[]").answering("version", 0, output);
        let error =
            require_supported_version(&host, &mut RecordedOutput::new()).refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::SbxVersionUnparseable));
    }

    let host = FakeSbx::listing("[]").answering("version", 1, "");
    let error = require_supported_version(&host, &mut RecordedOutput::new())
        .refused_because("a failed version command")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn a_missing_cli_is_reported_as_missing_without_running_anything() -> Checked {
    let host = FakeHost::new();
    let error = require_supported_version(&host, &mut RecordedOutput::new())
        .refused_because("sbx is not installed")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::HostCommandMissing);
    assert!(diagnostic.remediation.is_some());
    Ok(())
}

#[test]
fn the_cli_version_is_checked_before_the_sign_in_probe() -> Checked {
    let host = FakeSbx::listing("[]");
    let mut progress = RecordedOutput::new();
    require_ready(&host, &mut progress).required()?;
    assert_eq!(
        host.calls(),
        [
            vec!["version".to_string()],
            vec!["ls".to_string(), "--json".to_string()]
        ]
    );
    let steps: Vec<&str> = progress.steps.iter().map(|step| step.id).collect();
    assert_eq!(
        steps,
        ["progress-inspect-sbx-version", "progress-inspect-login"]
    );

    // 古いCLIでは、loginのprobeを読む前に止まる。
    let host = FakeSbx::listing("[]").answering("version", 0, "sbx version v0.47.0\n");
    let error = require_ready(&host, &mut RecordedOutput::new()).refused_because("0.47.0")?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxVersionBelowMinimum));
    assert_eq!(host.calls(), [vec!["version".to_string()]]);
    Ok(())
}

#[test]
fn the_fakes_answer_with_the_minimum_this_build_requires() -> Checked {
    // e2e testの代役は`tests/fixtures/sbx-version`を答える。最低versionを上げたのに
    // fixtureを合わせ忘れると、e2e testは古いCLIとして止まる理由を探すことになる。
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sbx-version"
    ))
    .required()?;
    for output in [fixture, supported_sbx_version()] {
        let host = FakeSbx::listing("[]").answering("version", 0, &output);
        require_supported_version(&host, &mut RecordedOutput::new()).required_because(&output)?;
        let older =
            crate::boundary::host::protocol::CliVersion::extract_from_output(&output).required()?;
        assert_eq!(older, crate::boundary::host::protocol::MINIMUM_CLI_VERSION);
    }
    Ok(())
}
