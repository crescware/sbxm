use super::*;
use crate::compatibility::EXPECTED_NETWORK_POLICY;
use crate::error::ErrorId;
use crate::i18n::Locale;
use crate::testing::global_status::{
    FakeHost, items, location_with_config, status_of, valid_config,
};
use crate::testing::render::plain;
use std::os::unix::fs::PermissionsExt;

#[test]
fn every_row_is_shown_in_the_documented_order_even_when_checks_fail() {
    let (_dir, location) = location_with_config(None);
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
}

#[test]
fn a_missing_configuration_is_the_defaults_rather_than_a_problem() {
    let (_dir, location) = location_with_config(None);
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(
        status_of(&status, "status-item-config"),
        StatusValue::Defaults
    );
    // 未作成のregistryは登録案件0件であり、errorではない。
    assert_eq!(
        status_of(&status, "status-item-registry"),
        StatusValue::Missing
    );
    assert_eq!(
        status_of(&status, "status-item-state-directory"),
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
}

#[test]
fn a_registry_that_cannot_be_read_is_diagnosed_without_visiting_any_project() {
    let (_dir, location) = location_with_config(None);
    std::fs::create_dir_all(location.dir()).unwrap();
    std::fs::write(location.registry_file(), "version: 99\nprojects: []\n").unwrap();
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-registry"),
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::RegistryUnknownVersion)
    );
}

#[test]
fn a_host_without_a_git_identity_cannot_register_a_new_project() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().failing("git config --global --get-all user.email", "", 1);

    let status = diagnose(&location, &host);
    assert_eq!(
        status_of(&status, "status-item-git-identity"),
        StatusValue::Missing
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GitIdentityUnavailable)
    );
}

#[test]
fn an_invalid_configuration_is_diagnosed_rather_than_repaired() {
    let (_dir, location) = location_with_config(Some("version: 99\n"));
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(status_of(&status, "status-item-config"), StatusValue::Error);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::ConfigUnknownVersion)
    );
}

#[test]
fn an_existing_state_directory_is_ready_and_a_missing_one_is_not_an_error() {
    let (_home, location) = location_with_config(Some(&valid_config()));
    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-state-directory"),
        StatusValue::Ready
    );
    assert_eq!(status_of(&status, "status-item-config"), StatusValue::Ready);
}

#[test]
fn the_platform_requirement_is_checked_against_the_observed_values() {
    let (_dir, location) = location_with_config(None);

    let host = FakeHost::macos();
    let status = diagnose(&location, &host);
    assert_eq!(
        status_of(&status, "status-item-platform"),
        StatusValue::Ready
    );

    let old = FakeHost::macos().responding("sw_vers -productVersion", "13.6\n");
    let status = diagnose(&location, &old);
    assert_eq!(
        status_of(&status, "status-item-platform"),
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
        status_of(&status, "status-item-platform"),
        StatusValue::Error
    );
}

#[test]
fn a_platform_that_cannot_be_observed_is_not_guessed() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::new().with_commands(&["git", "ssh", "docker", "sbx"]);
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-platform"),
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::PlatformUnobservable)
    );
}

#[test]
fn only_commands_that_sbxm_runs_directly_are_checked() {
    let (_dir, location) = location_with_config(None);
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
}

#[test]
fn a_missing_host_command_is_reported_with_an_install_hint() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().with_commands(&["ssh", "docker", "sbx"]);
    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-git"), StatusValue::Missing);
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::HostCommandMissing)
        .expect("the missing command is diagnosed");
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-install-command")
    );
}

#[test]
fn a_docker_engine_that_does_not_answer_is_an_error_with_the_original_stderr() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().failing(
        "docker version --format {{.Server.Version}}",
        "Cannot connect to the Docker daemon",
        1,
    );
    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-docker"), StatusValue::Error);
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::DockerUnreachable)
        .expect("an unreachable engine is diagnosed");
    let external = diagnostic
        .external
        .as_ref()
        .expect("the original stderr is preserved");
    assert!(external.stderr_text().contains("Cannot connect"));
}

#[test]
fn a_probe_timeout_is_an_error_rather_than_an_assumed_state() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().timing_out("docker version --format {{.Server.Version}}");
    let status = diagnose(&location, &host);

    assert_eq!(status_of(&status, "status-item-docker"), StatusValue::Error);
}

#[test]
fn a_version_below_the_minimum_stops_the_dependent_checks() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().responding("sbx version", "sbx version 0.36.9\n");
    let status = diagnose(&location, &host);

    for item in [
        "status-item-docker-sandboxes",
        "status-item-network-policy",
        "status-item-daemon",
    ] {
        assert_eq!(
            status_of(&status, item),
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
}

#[test]
fn sandbox_state_is_reported_from_the_structured_output() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos()
        .responding("sbx policy ls", r#"[{"name":"Balanced","active":true}]"#)
        .responding("sbx daemon status", "Status: running\n");
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes"),
        StatusValue::Ready
    );
    assert_eq!(
        status_of(&status, "status-item-network-policy"),
        StatusValue::Ready
    );
    assert_eq!(
        status_of(&status, "status-item-daemon"),
        StatusValue::Running
    );
}

#[test]
fn a_policy_that_is_not_the_expected_one_is_refused_even_when_stricter() {
    let (_dir, location) = location_with_config(None);

    for observed in ["Isolated", "Open"] {
        let host = FakeHost::macos()
            .responding(
                "sbx policy ls",
                &format!(r#"[{{"name":"{observed}","active":true}}]"#),
            )
            .responding("sbx daemon status", "Status: running\n");
        let status = diagnose(&location, &host);

        assert_eq!(
            status_of(&status, "status-item-network-policy"),
            StatusValue::Error,
            "{observed} is not {EXPECTED_NETWORK_POLICY}"
        );
        let diagnostic = status
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == ErrorId::NetworkPolicyMismatch)
            .unwrap_or_else(|| panic!("{observed} must be diagnosed: {:?}", status.diagnostics));
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
}

#[test]
fn a_version_that_cannot_be_parsed_stops_the_dependent_checks() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().responding("sbx version", "unreleased build\n");
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes"),
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SbxVersionUnparseable)
    );
}

#[test]
fn a_missing_sandboxes_cli_marks_every_dependent_row() {
    let (_dir, location) = location_with_config(None);
    let host = FakeHost::macos().with_commands(&["git", "ssh", "docker"]);
    let status = diagnose(&location, &host);

    assert_eq!(
        status_of(&status, "status-item-docker-sandboxes"),
        StatusValue::Missing
    );
    assert_eq!(status_of(&status, "status-item-daemon"), StatusValue::Error);
}

#[test]
fn several_problems_are_all_reported_at_once() {
    let (_dir, location) = location_with_config(None);
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
}

#[test]
fn the_rendered_report_shows_only_the_global_section() {
    let (_dir, location) = location_with_config(None);
    let status = diagnose(&location, &FakeHost::macos());
    let table = plain(
        &crate::commands::status::print::global_document(&status, Locale::En),
        Locale::En,
    );

    assert!(table.starts_with("GLOBAL\n"), "{table}");
    assert!(!table.contains("PROJECT"), "{table}");
    assert!(!table.contains("WORKTREES"), "{table}");
    assert_eq!(table.lines().count(), 2 + status.rows.len());
}

#[test]
fn the_report_never_touches_the_configuration_directory() {
    let (dir, location) = location_with_config(None);
    diagnose(&location, &FakeHost::macos());
    assert!(
        !location.dir().exists(),
        "a read-only diagnosis must not create {}",
        location.dir().display()
    );
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing may be written to the home directory"
    );
}

#[test]
fn a_state_directory_that_is_a_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let location = crate::config::ConfigLocation::from_home(dir.path().to_path_buf());
    std::fs::write(location.dir(), b"not a directory").unwrap();

    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-state-directory"),
        StatusValue::Error
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalStateUnusable)
    );
}
