use crate::boundary::host::protocol::CliVersion;
use crate::boundary::host::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::config::ConfigLocation;
use crate::diagnostics::ErrorId;
use crate::testing::global_status::{FakeHost, location_with_config, location_with_state_dir};
use crate::testing::host::{FakeSbx, reusable_executable, supported_git_version};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

/// `~/.sbxm`の無いhome。記録は読まれも書かれもしない。
fn no_state() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    location_with_config(None)
}

/// `git --version`だけに`output`を答えるhost。
fn answering(code: i32, output: &str) -> FakeSbx {
    FakeSbx::listing("[]").answering("--version", code, output)
}

/// Homebrewのgitを持つhost。
fn homebrew_git() -> FakeSbx {
    FakeSbx::listing("[]").with_executable("git", reusable_executable("git"))
}

fn steps(progress: &RecordedOutput) -> Vec<&'static str> {
    progress.steps.iter().map(|step| step.id).collect()
}

#[test]
fn the_minimum_version_is_read_with_a_captured_probe_and_shown_as_a_step() -> Checked {
    let (_dir, location) = no_state()?;
    let host = FakeSbx::listing("[]");
    let mut progress = RecordedOutput::new();
    require_supported_version(&location, &host, &mut progress).required()?;

    let spec = host.spec("--version")?;
    assert_eq!(spec.program, "git");
    assert_eq!(spec.args, ["--version"]);
    assert_eq!(spec.timeout, TimeoutClass::Probe);
    assert_eq!(spec.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(spec.output(), OutputPolicy::Capture);
    assert_eq!(host.calls().len(), 1);
    assert_eq!(steps(&progress), ["progress-inspect-git-version"]);
    Ok(())
}

#[test]
fn the_versions_hosts_print_are_read() -> Checked {
    let (_dir, location) = no_state()?;
    // macOSのgitは、versionの後にApple Gitの番号を続ける。
    for output in [
        "git version 2.39.5 (Apple Git-154)\n",
        "git version 2.36.0\n",
    ] {
        require_supported_version(&location, &answering(0, output), &mut RecordedOutput::new())
            .required_because(output)?;
    }
    Ok(())
}

#[test]
fn an_older_git_is_refused_with_the_version_it_needs() -> Checked {
    let (_dir, location) = no_state()?;
    let host = answering(0, "git version 2.32.1 (Apple Git-133)\n");
    let error = require_supported_version(&location, &host, &mut RecordedOutput::new())
        .refused_because("2.32.1")?;
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
    let (_dir, location) = no_state()?;
    for output in ["", "git version unknown\n"] {
        let error =
            require_supported_version(&location, &answering(0, output), &mut RecordedOutput::new())
                .refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::HostGitVersionUnparseable));
    }

    let error = require_supported_version(&location, &answering(1, ""), &mut RecordedOutput::new())
        .refused_because("a failed version command")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn a_missing_git_is_reported_as_missing_without_running_anything() -> Checked {
    let (_dir, location) = no_state()?;
    let mut progress = RecordedOutput::new();
    let error = require_supported_version(&location, &FakeHost::new(), &mut progress)
        .refused_because("git is not installed")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::HostCommandMissing);
    assert!(diagnostic.remediation.is_some());
    assert!(progress.steps.is_empty(), "nothing to check without git");
    Ok(())
}

#[test]
fn a_recorded_executable_skips_the_probe_and_its_step_until_it_changes() -> Checked {
    let (_dir, location) = location_with_state_dir()?;

    // 初回は読み、通れば記録する。
    let host = homebrew_git();
    let mut progress = RecordedOutput::new();
    require_supported_version(&location, &host, &mut progress).required()?;
    assert_eq!(host.calls(), [vec!["--version".to_string()]]);
    assert_eq!(steps(&progress), ["progress-inspect-git-version"]);
    assert!(location.host_tools_file().is_file());

    // 同じ実行ファイルなら読まず、工程も示さない。
    let host = homebrew_git();
    let mut progress = RecordedOutput::new();
    require_supported_version(&location, &host, &mut progress).required()?;
    assert!(host.calls().is_empty());
    assert!(progress.steps.is_empty());
    let trace = progress.traces.first().required()?;
    assert_eq!(trace.id, "trace-host-tool-skipped");
    assert!(trace.args.contains(&("program", "git".to_string())));

    // 実行ファイルが変われば読み直す。
    let mut upgraded = reusable_executable("git");
    upgraded.identity.ctime_secs += 1;
    let host = FakeSbx::listing("[]").with_executable("git", upgraded);
    let mut progress = RecordedOutput::new();
    require_supported_version(&location, &host, &mut progress).required()?;
    assert_eq!(host.calls(), [vec!["--version".to_string()]]);
    assert_eq!(steps(&progress), ["progress-inspect-git-version"]);
    Ok(())
}

#[test]
fn apples_git_shim_is_checked_every_time() -> Checked {
    let (_dir, location) = location_with_state_dir()?;
    let mut shim = reusable_executable("git");
    shim.identity.path = std::path::PathBuf::from("/usr/bin/git");
    for _ in 0..2 {
        let host = FakeSbx::listing("[]").with_executable("git", shim.clone());
        let mut progress = RecordedOutput::new();
        require_supported_version(&location, &host, &mut progress).required()?;
        assert_eq!(host.calls(), [vec!["--version".to_string()]]);
        assert_eq!(steps(&progress), ["progress-inspect-git-version"]);
    }
    assert!(!location.host_tools_file().exists());
    Ok(())
}

#[test]
fn a_git_that_fails_the_check_is_not_recorded() -> Checked {
    let (_dir, location) = location_with_state_dir()?;
    let host = homebrew_git().answering("--version", 0, "git version 2.32.1\n");
    require_supported_version(&location, &host, &mut RecordedOutput::new())
        .refused_because("2.32.1")?;
    assert!(!location.host_tools_file().exists());
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
