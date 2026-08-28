use crate::support::StatusValue;

use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::boundary::host::protocol::EXPECTED_NETWORK_POLICY;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId, Msg};
use crate::i18n::Locale;
use crate::testing::global_status::{
    FakeHost, items, location_with_config, status_of, valid_config,
};
use crate::testing::plain;
use std::os::unix::fs::PermissionsExt;

#[test]
fn every_row_is_shown_in_the_documented_order_even_when_checks_fail() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let status = diagnose(&location, &FakeHost::new());

    assert_eq!(
        items(&status),
        vec![
            "status-item-state-directory",
            "status-item-config",
            "status-item-registry",
            "status-item-platform",
            "status-item-git",
            "status-item-ssh",
            "status-item-docker",
            "status-item-git-identity",
            "status-item-docker-sandboxes",
            "status-item-network-policy",
            "status-item-daemon",
            "status-item-login",
            "status-item-remote-ssh",
        ]
    );
    Ok(())
}

#[test]
fn a_missing_configuration_is_the_defaults_rather_than_a_problem() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(
        status_of(&status, "status-item-config")?,
        StatusValue::Defaults
    );
    // 未作成のregistryは登録案件0件であり、errorではない。
    assert_eq!(
        status_of(&status, "status-item-registry")?,
        StatusValue::Missing
    );
    assert_eq!(
        status_of(&status, "status-item-state-directory")?,
        StatusValue::Missing
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalStateUnusable),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_registry_that_cannot_be_read_is_diagnosed_without_visiting_any_project() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    std::fs::create_dir_all(location.dir()).required()?;
    std::fs::write(location.registry_file(), "version: 99\nprojects: []\n").required()?;
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o600),
    )
    .required()?;

    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-registry")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::RegistryUnknownVersion)
    );
    Ok(())
}

#[test]
fn a_registry_that_holds_entries_is_ready_without_reading_any_of_them() -> Checked {
    use crate::testing::registry::{Entry, document};

    let (_dir, location) = location_with_config(None)?;
    std::fs::create_dir_all(location.dir()).required()?;
    std::fs::write(
        location.registry_file(),
        document(&[Entry::of(
            "example-org/example-repo",
            "/home/example/Projects/example-repo.project",
            "ssh",
        )]),
    )
    .required()?;
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o600),
    )
    .required()?;

    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-registry")?,
        StatusValue::Ready
    );
    // 登録案件のproject rootはこのhostに無い。案件へ触れないからこそReadyになる。
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id.as_str().starts_with("registry-")),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn an_unchosen_git_identity_is_reported_as_missing_rather_than_as_a_fault() -> Checked {
    // hostが何を宣言していようと、既定を選ぶのは利用者である。まだ選んでいないことは
    // 診断すべき異常ではなく、対話的な`add`がまだ一度も訊いていないだけである。
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing("git config --global --get-all user.email", "", 1);

    let status = diagnose(&location, &host);
    assert_eq!(
        status_of(&status, "status-item-git-identity")?,
        StatusValue::Missing
    );
    assert!(
        !status.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.id,
            ErrorId::GitIdentityUndecidable | ErrorId::GitIdentityIncomplete
        )),
        "an unchosen identity produces no diagnostic: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_chosen_git_identity_is_read_from_the_configuration_rather_than_the_host() -> Checked {
    let (_dir, location) = location_with_config(Some(
        "version: 1\ngit_user_name: Example User\ngit_user_email: user@example.com\n",
    ))?;
    // hostが答えられなくても、保存済みの既定はそのまま使える。
    let host = FakeHost::macos().failing("git config --global --get-all user.name", "", 1);

    let status = diagnose(&location, &host);
    assert_eq!(
        status_of(&status, "status-item-git-identity")?,
        StatusValue::Ready
    );
    Ok(())
}

#[test]
fn an_invalid_configuration_is_diagnosed_rather_than_repaired() -> Checked {
    let (_dir, location) = location_with_config(Some("version: 99\n"))?;
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(
        status_of(&status, "status-item-config")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::ConfigUnknownVersion)
    );
    Ok(())
}

#[test]
fn an_existing_state_directory_is_ready_and_a_missing_one_is_not_an_error() -> Checked {
    let (_home, location) = location_with_config(Some(&valid_config()))?;
    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-state-directory")?,
        StatusValue::Ready
    );
    assert_eq!(
        status_of(&status, "status-item-config")?,
        StatusValue::Ready
    );
    Ok(())
}

#[test]
fn the_platform_requirement_is_checked_against_the_observed_values() -> Checked {
    let (_dir, location) = location_with_config(None)?;

    let host = FakeHost::macos();
    let status = diagnose(&location, &host);
    assert_eq!(
        status_of(&status, "status-item-platform")?,
        StatusValue::Ready
    );

    let old = FakeHost::macos().responding("sw_vers -productVersion", "13.6\n");
    let status = diagnose(&location, &old);
    assert_eq!(
        status_of(&status, "status-item-platform")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::PlatformUnsupported)
    );

    let intel = FakeHost::macos().responding("uname -m", "x86_64\n");
    let status = diagnose(&location, &intel);
    assert_eq!(
        status_of(&status, "status-item-platform")?,
        StatusValue::Error
    );
    Ok(())
}

#[test]
fn a_platform_that_cannot_be_observed_is_not_guessed() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::new().with_commands(&["git", "ssh", "docker", "sbx"]);
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-platform")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::PlatformUnobservable)
    );
    Ok(())
}

#[test]
fn only_commands_that_sbxm_runs_directly_are_checked() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    // 利用者が実務で使う可能性があっても、sbxmが直接使わないtoolは検査しない。
    let host = FakeHost::macos();
    let status = diagnose(&location, &host);
    assert!(
        !items(&status)
            .iter()
            .any(|item| item.contains("mise") || item.contains("brew") || item.contains("gh")),
        "{:?}",
        items(&status)
    );
    Ok(())
}

#[test]
fn a_missing_host_command_is_reported_with_an_install_hint() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().with_commands(&["ssh", "docker", "sbx"]);
    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-git")?, StatusValue::Missing);
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::HostCommandMissing)
        .required_because("the missing command is diagnosed")?;
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-install-command")
    );
    Ok(())
}

#[test]
fn a_docker_engine_that_does_not_answer_is_an_error_with_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing(
        "docker version --format {{.Server.Version}}",
        "Cannot connect to the Docker daemon",
        1,
    );
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker")?,
        StatusValue::Error
    );
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::DockerUnreachable)
        .required_because("an unreachable engine is diagnosed")?;
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(external.stderr_text().contains("Cannot connect"));
    Ok(())
}

#[test]
fn a_docker_client_that_answers_without_a_server_version_is_not_read_as_ready() -> Checked {
    // clientだけが答えた場合、`docker version`は成功したままserver版を空で返す。
    // 成功したことをengineが動いている証拠として読まない。
    let (_dir, location) = location_with_config(None)?;
    for observed in ["", "\n", "   \n"] {
        let host =
            FakeHost::macos().responding("docker version --format {{.Server.Version}}", observed);
        let status = diagnose(&location, &host);

        assert_eq!(
            status_of(&status, "status-item-docker")?,
            StatusValue::Error,
            "{observed:?} is not a server version"
        );
        let diagnostic = diagnosed(&status, ErrorId::DockerUnreachable)?;
        assert_eq!(reason(diagnostic)?.id, "cause-server-version-empty");
        assert_eq!(
            remediation_ids(diagnostic),
            vec!["remediation-start-docker"]
        );
        // 失敗していない実行にはexternalが無い。無い事実を作らない。
        assert!(diagnostic.external.is_none(), "{:?}", diagnostic.external);
    }
    Ok(())
}

#[test]
fn a_probe_timeout_is_an_error_rather_than_an_assumed_state() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().timing_out("docker version --format {{.Server.Version}}");
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker")?,
        StatusValue::Error
    );
    Ok(())
}

#[test]
fn a_version_below_the_minimum_stops_the_dependent_checks() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx version", "sbx version 0.36.9\n");
    let status = diagnose(&location, &host);

    for item in [
        "status-item-docker-sandboxes",
        "status-item-network-policy",
        "status-item-daemon",
    ] {
        assert_eq!(
            status_of(&status, item)?,
            StatusValue::Error,
            "{item} must not be observed through an unsupported CLI"
        );
    }
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SbxVersionBelowMinimum),
        "the refused version must be diagnosed"
    );
    Ok(())
}

#[test]
fn sandbox_state_is_reported_from_the_structured_output() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos()
        .responding("sbx policy ls", r#"[{"name":"Balanced","active":true}]"#)
        .responding("sbx daemon status", "Status: running\n");
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes")?,
        StatusValue::Ready
    );
    assert_eq!(
        status_of(&status, "status-item-network-policy")?,
        StatusValue::Ready
    );
    assert_eq!(
        status_of(&status, "status-item-daemon")?,
        StatusValue::Running
    );
    Ok(())
}

#[test]
fn a_policy_that_is_not_the_expected_one_is_refused_even_when_stricter() -> Checked {
    let (_dir, location) = location_with_config(None)?;

    for observed in ["Isolated", "Open"] {
        let host = FakeHost::macos()
            .responding(
                "sbx policy ls",
                &format!(r#"[{{"name":"{observed}","active":true}}]"#),
            )
            .responding("sbx daemon status", "Status: running\n");
        let status = diagnose(&location, &host);

        assert_eq!(
            status_of(&status, "status-item-network-policy")?,
            StatusValue::Error,
            "{observed} is not {EXPECTED_NETWORK_POLICY}"
        );
        let diagnostic = status
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == ErrorId::NetworkPolicyMismatch)
            .required_because(&format!(
                "{observed} must be diagnosed: {:?}",
                status.diagnostics
            ))?;
        assert!(
            diagnostic
                .description
                .args
                .contains(&("observed", observed.to_string())),
            "{:?}",
            diagnostic.description.args
        );
        assert!(
            diagnostic
                .description
                .args
                .contains(&("expected", EXPECTED_NETWORK_POLICY.to_string())),
            "{:?}",
            diagnostic.description.args
        );
    }
    Ok(())
}

#[test]
fn a_version_that_cannot_be_parsed_stops_the_dependent_checks() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx version", "unreleased build\n");
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SbxVersionUnparseable)
    );
    Ok(())
}

#[test]
fn a_sandboxes_cli_that_refuses_to_report_its_version_keeps_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing("sbx version", "sbx: unknown command \"version\"", 1);
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::SbxVersionUnparseable)?;
    // 版が読めなかった理由は、sbxm自身の言葉ではなくsbxが書いた通りに残す。
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(
        external.stderr_text().contains("unknown command"),
        "{:?}",
        external.stderr_text()
    );
    assert!(
        diagnostic.description.args.contains(&(
            "observed",
            ErrorId::ExternalCommandFailed.as_str().to_string()
        )),
        "{:?}",
        diagnostic.description.args
    );
    for item in ["status-item-daemon", "status-item-remote-ssh"] {
        assert_eq!(
            status_of(&status, item)?,
            StatusValue::Error,
            "{item} must not be observed through a CLI that did not answer"
        );
    }
    Ok(())
}

#[test]
fn a_missing_sandboxes_cli_marks_every_dependent_row() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().with_commands(&["git", "ssh", "docker"]);
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes")?,
        StatusValue::Missing
    );
    assert_eq!(
        status_of(&status, "status-item-daemon")?,
        StatusValue::Error
    );
    Ok(())
}

#[test]
fn several_problems_are_all_reported_at_once() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::new().with_commands(&["sbx"]);
    let status = diagnose(&location, &host);

    let ids: Vec<ErrorId> = status
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id)
        .collect();
    assert!(ids.contains(&ErrorId::PlatformUnobservable), "{ids:?}");
    assert!(ids.contains(&ErrorId::HostCommandMissing), "{ids:?}");
    assert!(!status.is_healthy());
    Ok(())
}

#[test]
fn the_rendered_report_shows_only_the_global_section() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let status = diagnose(&location, &FakeHost::macos());
    let table = plain(
        &crate::commands::status::print::global_document(&status, Locale::En),
        Locale::En,
    )?;

    assert!(table.starts_with("GLOBAL\n"), "{table}");
    assert!(!table.contains("PROJECT"), "{table}");
    assert!(!table.contains("WORKTREES"), "{table}");
    assert_eq!(table.lines().count(), 2 + status.rows.len());
    Ok(())
}

#[test]
fn the_report_never_touches_the_configuration_directory() -> Checked {
    let (dir, location) = location_with_config(None)?;
    diagnose(&location, &FakeHost::macos());
    assert!(
        !location.dir().exists(),
        "a read-only diagnosis must not create {}",
        location.dir().display()
    );
    assert_eq!(
        std::fs::read_dir(dir.path()).required()?.count(),
        0,
        "nothing may be written to the home directory"
    );
    Ok(())
}

#[test]
fn a_state_directory_that_is_a_file_is_an_error() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let location = crate::config::ConfigLocation::from_home(dir.path().to_path_buf());
    std::fs::write(location.dir(), b"not a directory").required()?;

    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-state-directory")?,
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalStateUnusable)
    );
    Ok(())
}

/// 期待したidの診断1件。出ていない場合は、代わりに出た診断を添えて失敗する。
fn diagnosed(status: &GlobalStatus, id: ErrorId) -> Checked<&Diagnostic> {
    let reported: Vec<ErrorId> = status
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id)
        .collect();
    status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == id)
        .required_because(&format!("{id} must be diagnosed, but {reported:?} was"))
}

/// 項目名で引いた1行の事実。翻訳しない値をそのまま返す。
fn fact(diagnostic: &Diagnostic, label: &str) -> Checked<String> {
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label: name, value } if name.id == label => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because(&format!("the diagnosis carries {label}"))
}

/// 観測を止めたものを原文のまま示す`Cause:`。
fn cause(diagnostic: &Diagnostic) -> Checked<String> {
    fact(diagnostic, "diagnostic-cause-label")
}

/// sbxm自身が観測したことを翻訳して示す`Cause:`。
fn reason(diagnostic: &Diagnostic) -> Checked<&Msg> {
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::Translated { value, .. } => Some(value),
            _ => None,
        })
        .required_because("the diagnosis states the cause in a translated sentence")
}

/// 利用者へ見せる対処方法のmessage id。
fn remediation_ids(diagnostic: &Diagnostic) -> Vec<&'static str> {
    diagnostic
        .remediation
        .iter()
        .flat_map(|remediation| remediation.explanation.iter())
        .map(|explanation| explanation.id)
        .collect()
}

/// 利用者へ見せる対処方法のcommand行。
fn remediation_commands(diagnostic: &Diagnostic) -> Vec<&str> {
    diagnostic
        .remediation
        .iter()
        .flat_map(|remediation| remediation.commands.iter())
        .map(crate::design::text::CommandLine::as_str)
        .collect()
}

#[test]
fn a_macos_version_that_is_not_a_number_is_unobservable_rather_than_unsupported() -> Checked {
    // 読めなかった版を対応外だと言い切ると、利用者はupgradeを促されるだけで、
    // 本当の原因である「版を読めていないこと」に辿り着けない。
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sw_vers -productVersion", "beta\n");

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-platform")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::PlatformUnobservable)?;
    let reason = reason(diagnostic)?;
    assert_eq!(reason.id, "cause-macos-version-unreadable");
    assert!(
        reason.args.contains(&("observed", "beta".to_string())),
        "the value that could not be read is shown: {:?}",
        reason.args
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::PlatformUnsupported),
        "an unread version is not a refused version: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_state_directory_that_cannot_be_written_is_an_error_that_names_the_path() -> Checked {
    // 読めることは使えることではない。登録も更新も書き込みを要するため、
    // 書けないdirectoryをreadyとしない。
    let dir = tempfile::tempdir().required()?;
    let location = crate::config::ConfigLocation::from_home(dir.path().to_path_buf());
    std::fs::create_dir_all(location.dir()).required()?;
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o500)).required()?;

    let status = diagnose(&location, &FakeHost::macos());

    // 後片付けができるよう、観測が終わったら戻す。
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).required()?;
    assert_eq!(
        status_of(&status, "status-item-state-directory")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::GlobalStateUnusable)?;
    assert_eq!(reason(diagnostic)?.id, "cause-not-writable");
    assert_eq!(
        fact(diagnostic, "diagnostic-path-label")?,
        crate::paths::display(&location.dir())
    );
    Ok(())
}

#[test]
fn a_signed_in_host_is_ready_and_says_nothing_further() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx login status --json", r#"{"logged_in":true}"#);

    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-login")?, StatusValue::Ready);
    assert!(
        !status.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.id,
            ErrorId::SbxLoginMissing | ErrorId::SbxLoginUnobservable
        )),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_host_that_is_not_signed_in_is_missing_and_is_shown_the_command_that_signs_in() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx login status --json", r#"{"logged_in":false}"#);

    let status = diagnose(&location, &host);

    // loginしていないことは観測できた事実であり、故障ではない。
    assert_eq!(
        status_of(&status, "status-item-login")?,
        StatusValue::Missing
    );
    let diagnostic = diagnosed(&status, ErrorId::SbxLoginMissing)?;
    assert_eq!(remediation_ids(diagnostic), vec!["remediation-sbx-login"]);
    assert_eq!(remediation_commands(diagnostic), vec!["sbx login"]);
    Ok(())
}

#[test]
fn login_output_that_states_nothing_about_the_session_is_not_read_as_signed_in() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx login status --json", r#"{"user":"someone"}"#);

    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-login")?, StatusValue::Error);
    // 解釈できない出力からlogin済みを推測しないため、parseの失敗をそのまま伝える。
    let diagnostic = diagnosed(&status, ErrorId::ExternalOutputUnparseable)?;
    assert_eq!(
        fact(diagnostic, "diagnostic-command-label")?,
        "sbx login status"
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SbxLoginMissing),
        "an unreadable answer is not an answer of no: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_login_probe_that_exits_non_zero_is_unobservable_and_keeps_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing(
        "sbx login status --json",
        "Error: the daemon is not running",
        1,
    );

    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-login")?, StatusValue::Error);
    let diagnostic = diagnosed(&status, ErrorId::SbxLoginUnobservable)?;
    assert_eq!(cause(diagnostic)?, ErrorId::ExternalCommandFailed.as_str());
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(
        external.stderr_text().contains("the daemon is not running"),
        "{:?}",
        external.stderr_text()
    );
    Ok(())
}

#[test]
fn a_login_probe_that_times_out_is_unobservable_rather_than_signed_in() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().timing_out("sbx login status --json");

    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-login")?, StatusValue::Error);
    let diagnostic = diagnosed(&status, ErrorId::SbxLoginUnobservable)?;
    // 答えが返らなかったことも、答えとして記録する。
    assert_eq!(cause(diagnostic)?, ErrorId::ExternalCommandTimeout.as_str());
    Ok(())
}

#[test]
fn a_stopped_daemon_is_reported_as_stopped_rather_than_as_a_failure() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx daemon status", "Status: stopped\n");

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-daemon")?,
        StatusValue::Stopped
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::DaemonUnobservable),
        "a daemon that answers is observed: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_daemon_state_with_no_defined_meaning_is_not_read_as_running() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding("sbx daemon status", "Status: degraded\n");

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-daemon")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::DaemonUnobservable)?;
    assert_eq!(
        cause(diagnostic)?,
        ErrorId::ExternalOutputUnparseable.as_str()
    );
    Ok(())
}

#[test]
fn a_daemon_probe_that_exits_non_zero_keeps_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing("sbx daemon status", "Cannot reach the daemon socket", 1);

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-daemon")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::DaemonUnobservable)?;
    assert_eq!(cause(diagnostic)?, ErrorId::ExternalCommandFailed.as_str());
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(
        external.stderr_text().contains("Cannot reach"),
        "{:?}",
        external.stderr_text()
    );
    Ok(())
}

#[test]
fn a_policy_listing_that_marks_nothing_active_is_unobservable_rather_than_a_mismatch() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos()
        .responding("sbx policy ls", r#"[{"name":"Balanced","active":false}]"#)
        .responding("sbx daemon status", "Status: running\n");

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-network-policy")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::NetworkPolicyUnobservable)?;
    assert_eq!(
        cause(diagnostic)?,
        ErrorId::ExternalOutputUnparseable.as_str()
    );
    // 現在値を読めていない以上、期待値と違うとは言えない。
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::NetworkPolicyMismatch),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_policy_probe_that_exits_non_zero_keeps_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing("sbx policy ls", "Error: no such command", 127);

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-network-policy")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::NetworkPolicyUnobservable)?;
    assert_eq!(cause(diagnostic)?, ErrorId::ExternalCommandFailed.as_str());
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(
        external.stderr_text().contains("no such command"),
        "{:?}",
        external.stderr_text()
    );
    Ok(())
}

#[test]
fn an_ssh_configuration_that_routes_the_sandbox_domain_is_ready() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    // `ssh -G`は接続せず実効設定だけを答える。ProxyCommandの有無が経路の有無である。
    let host = FakeHost::macos().responding(
        "ssh -G sbxm-probe.sbx",
        "user example\nhostname sbxm-probe.sbx\nProxyCommand sbx ssh-proxy %h\n",
    );

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-remote-ssh")?,
        StatusValue::Ready
    );
    assert!(
        !status.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.id,
            ErrorId::RemoteSshUnconfigured | ErrorId::RemoteSshUnobservable
        )),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn ssh_that_answers_without_a_proxy_is_missing_and_is_told_to_set_the_integration_up() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().responding(
        "ssh -G sbxm-probe.sbx",
        "user example\nhostname sbxm-probe.sbx\nport 22\n",
    );

    let status = diagnose(&location, &host);

    // sshは答えているため観測は成立している。足りないのは設定である。
    assert_eq!(
        status_of(&status, "status-item-remote-ssh")?,
        StatusValue::Missing
    );
    let diagnostic = diagnosed(&status, ErrorId::RemoteSshUnconfigured)?;
    assert!(
        diagnostic
            .description
            .args
            .contains(&("host", "*.sbx".to_string())),
        "the domain that has no route is named: {:?}",
        diagnostic.description.args
    );
    assert_eq!(
        remediation_ids(diagnostic),
        vec!["remediation-remote-ssh-unconfigured"]
    );
    // 実機で確認していないsetup commandは案内しない。
    assert!(
        remediation_commands(diagnostic).is_empty(),
        "{:?}",
        remediation_commands(diagnostic)
    );
    Ok(())
}

#[test]
fn an_ssh_probe_that_exits_non_zero_is_unobservable_and_keeps_the_original_stderr() -> Checked {
    let (_dir, location) = location_with_config(None)?;
    let host = FakeHost::macos().failing(
        "ssh -G sbxm-probe.sbx",
        "/home/example/.ssh/config: line 3: Bad configuration option",
        255,
    );

    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-remote-ssh")?,
        StatusValue::Error
    );
    let diagnostic = diagnosed(&status, ErrorId::RemoteSshUnobservable)?;
    assert_eq!(cause(diagnostic)?, ErrorId::ExternalCommandFailed.as_str());
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original stderr is preserved")?;
    assert!(
        external.stderr_text().contains("Bad configuration option"),
        "{:?}",
        external.stderr_text()
    );
    // 設定の不足だと言い切れないため、未設定としては報告しない。
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::RemoteSshUnconfigured),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}
