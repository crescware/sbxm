use crate::boundary::host::protocol::CliVersion;
use crate::boundary::host::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::diagnostics::ErrorId;
use crate::testing::global_status::FakeHost;
use crate::testing::host::{FakeSbx, supported_git_version};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

/// `git --version`だけに`output`を答えるhost。
fn answering(code: i32, output: &str) -> FakeSbx {
    FakeSbx::listing("[]").answering("--version", code, output)
}

#[test]
fn the_minimum_version_is_read_with_a_captured_probe_and_shown_as_a_step() -> Checked {
    let host = FakeSbx::listing("[]");
    let mut progress = RecordedOutput::new();
    require_supported_version(&host, &mut progress).required()?;

    let spec = host.spec("--version")?;
    assert_eq!(spec.program, "git");
    assert_eq!(spec.args, ["--version"]);
    assert_eq!(spec.timeout, TimeoutClass::Probe);
    assert_eq!(spec.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(spec.output(), OutputPolicy::Capture);
    assert_eq!(host.calls().len(), 1);
    assert_eq!(progress.steps.len(), 1);
    assert_eq!(progress.steps[0].id, "progress-inspect-git-version");
    Ok(())
}

#[test]
fn the_versions_hosts_print_are_read() -> Checked {
    // macOSのgitは、versionの後にApple Gitの番号を続ける。
    for output in [
        "git version 2.39.5 (Apple Git-154)\n",
        "git version 2.36.0\n",
    ] {
        require_supported_version(&answering(0, output), &mut RecordedOutput::new())
            .required_because(output)?;
    }
    Ok(())
}

#[test]
fn an_older_git_is_refused_with_the_version_it_needs() -> Checked {
    let host = answering(0, "git version 2.32.1 (Apple Git-133)\n");
    let error =
        require_supported_version(&host, &mut RecordedOutput::new()).refused_because("2.32.1")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::HostGitTooOld);
    assert!(
        diagnostic
            .description
            .args
            .contains(&("minimum", MINIMUM_HOST_GIT.to_string()))
    );
    let remediation = diagnostic.remediation.as_ref().required()?;
    assert_eq!(
        remediation.explanation[0].id,
        "remediation-host-git-too-old"
    );
    Ok(())
}

#[test]
fn a_version_that_cannot_be_read_is_not_taken_for_a_supported_one() -> Checked {
    for output in ["", "git version unknown\n"] {
        let error = require_supported_version(&answering(0, output), &mut RecordedOutput::new())
            .refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::HostGitVersionUnparseable));
    }

    let error = require_supported_version(&answering(1, ""), &mut RecordedOutput::new())
        .refused_because("a failed version command")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn a_missing_git_is_reported_as_missing_without_running_anything() -> Checked {
    let error = require_supported_version(&FakeHost::new(), &mut RecordedOutput::new())
        .refused_because("git is not installed")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::HostCommandMissing);
    assert!(diagnostic.remediation.is_some());
    Ok(())
}

#[test]
fn the_fakes_answer_with_the_minimum_this_build_requires() -> Checked {
    // e2e testの代役は`tests/fixtures/git-version`を答える。最低versionを上げたのに
    // fixtureを合わせ忘れると、e2e testは古いgitとして止まる理由を探すことになる。
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/git-version"
    ))
    .required()?;
    for output in [fixture, supported_git_version()] {
        assert_eq!(
            CliVersion::extract_from_output(&output).required_because(&output)?,
            MINIMUM_HOST_GIT
        );
    }
    Ok(())
}

#[test]
fn the_documented_minimum_is_the_one_this_build_requires() -> Checked {
    // 最低versionはこのconstantだけで決める。文書は末尾の`.0`を省いて書く。
    let stated = if MINIMUM_HOST_GIT.patch == 0 {
        format!("Git {}.{}", MINIMUM_HOST_GIT.major, MINIMUM_HOST_GIT.minor)
    } else {
        format!("Git {MINIMUM_HOST_GIT}")
    };
    for document in [
        "README.md",
        "docs/README.ja.md",
        "website/src/content/docs/getting-started/requirements.md",
        "website/src/content/docs/index.mdx",
    ] {
        let text = std::fs::read_to_string(format!("{}/{document}", env!("CARGO_MANIFEST_DIR")))
            .required_because(document)?;
        assert!(text.contains(&stated), "{document} must state {stated}");
    }
    Ok(())
}
