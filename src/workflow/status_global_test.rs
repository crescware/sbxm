use super::*;
use crate::command::CommandOutcome;
use crate::i18n::{Catalog, Locale};
use crate::workflow::Reporter;
use std::collections::HashMap;

struct FakeHost {
    present: Vec<String>,
    responses: HashMap<String, std::result::Result<(String, String, i32), ErrorId>>,
}

impl FakeHost {
    fn new() -> FakeHost {
        FakeHost {
            present: Vec::new(),
            responses: HashMap::new(),
        }
    }

    fn with_commands(mut self, programs: &[&str]) -> FakeHost {
        self.present = programs.iter().map(|value| value.to_string()).collect();
        self
    }

    fn responding(mut self, key: &str, stdout: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Ok((stdout.to_string(), String::new(), 0)));
        self
    }

    fn failing(mut self, key: &str, stderr: &str, code: i32) -> FakeHost {
        self.responses.insert(
            key.to_string(),
            Ok((String::new(), stderr.to_string(), code)),
        );
        self
    }

    fn timing_out(mut self, key: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Err(ErrorId::ExternalCommandTimeout));
        self
    }

    fn macos() -> FakeHost {
        FakeHost::new()
            .with_commands(&["git", "ssh", "docker", "sbx"])
            .responding("sw_vers -productVersion", "14.5\n")
            .responding("uname -m", "arm64\n")
            .responding("docker version --format {{.Server.Version}}", "27.0.3\n")
            .responding("sbx version", "sbx version 0.37.0\n")
    }
}

impl HostEnvironment for FakeHost {
    fn command_exists(&self, program: &str) -> bool {
        self.present.iter().any(|value| value == program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let key = if spec.args.is_empty() {
            spec.program.clone()
        } else {
            format!("{} {}", spec.program, spec.args.join(" "))
        };
        match self.responses.get(&key) {
            Some(Ok((stdout, stderr, code))) => Ok(crate::testing::command::outcome_with_stderr(
                spec, *code, stdout, stderr,
            )),
            Some(Err(ErrorId::ExternalCommandTimeout)) => Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            )),
            Some(Err(id)) => Err(Error::new(
                *id,
                msg!("error-external-command-not-found", program = spec.program),
            )),
            None => Err(Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )),
        }
    }
}

fn location_with_config(text: Option<&str>) -> (tempfile::TempDir, ConfigLocation) {
    let dir = tempfile::tempdir().expect("temporary home");
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    if let Some(text) = text {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(location.dir()).unwrap();
        std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(location.config_file(), text).unwrap();
        std::fs::set_permissions(
            location.config_file(),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    (dir, location)
}

fn valid_config(base: &Path) -> String {
    format!(
        "version = 1\nlanguage = \"en\"\nbase_path = \"{}\"\n\n[git]\nuser_name = \"Example User\"\nuser_email = \"user@example.com\"\n",
        base.display()
    )
}

fn items(status: &GlobalStatus) -> Vec<&'static str> {
    status.rows.iter().map(|row| row.item).collect()
}

fn status_of(status: &GlobalStatus, item: &str) -> StatusValue {
    status
        .rows
        .iter()
        .find(|row| row.item == item)
        .unwrap_or_else(|| panic!("row {item} is missing"))
        .status
}

#[test]
fn every_row_is_shown_in_the_documented_order_even_when_checks_fail() {
    let (_dir, location) = location_with_config(None);
    let status = diagnose(&location, &FakeHost::new());

    assert_eq!(
        items(&status),
        vec![
            "status-item-config",
            "status-item-base-path",
            "status-item-platform",
            "status-item-git",
            "status-item-ssh",
            "status-item-docker",
            "status-item-docker-sandboxes",
            "status-item-network-policy",
            "status-item-daemon",
            "status-item-login",
            "status-item-remote-ssh",
        ]
    );
}

#[test]
fn a_missing_configuration_points_at_init_without_stopping_the_other_checks() {
    let (_dir, location) = location_with_config(None);
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(
        status_of(&status, "status-item-config"),
        StatusValue::Missing
    );
    assert_eq!(status_of(&status, "status-item-git"), StatusValue::Ready);
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::ConfigMissing)
        .expect("the missing configuration is diagnosed");
    assert_eq!(
        diagnostic.remediation.as_ref().map(|message| message.id),
        Some("remediation-run-init")
    );
}

#[test]
fn an_invalid_configuration_is_diagnosed_rather_than_repaired() {
    let (_dir, location) = location_with_config(Some("version = 99\n"));
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
fn an_existing_base_path_is_ready_and_a_missing_one_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("Projects");
    std::fs::create_dir(&base).unwrap();
    let (_home, location) = location_with_config(Some(&valid_config(&base)));
    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-base-path"),
        StatusValue::Ready
    );

    let absent = dir.path().join("NotYet");
    let (_home, location) = location_with_config(Some(&valid_config(&absent)));
    let status = diagnose(&location, &FakeHost::macos());
    assert_eq!(
        status_of(&status, "status-item-base-path"),
        StatusValue::Missing
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::BasePathNotDirectory)
    );
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
        diagnostic.remediation.as_ref().map(|message| message.id),
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
    assert!(ids.contains(&ErrorId::ConfigMissing), "{ids:?}");
    assert!(ids.contains(&ErrorId::PlatformUnobservable), "{ids:?}");
    assert!(ids.contains(&ErrorId::HostCommandMissing), "{ids:?}");
    assert!(!status.is_healthy());
}

#[test]
fn the_rendered_report_shows_only_the_global_section() {
    let (_dir, location) = location_with_config(None);
    let status = diagnose(&location, &FakeHost::macos());
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let table = reporter.render_status_table(
        "status-global-section",
        "status-column-item",
        "status-column-status",
        &status.rows,
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
fn a_base_path_that_is_a_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Projects");
    std::fs::write(&file, b"not a directory").unwrap();
    // configのvalidationがfileを拒否するため、診断はconfig側に現れる。
    let (_home, location) = location_with_config(Some(&valid_config(&file)));
    let status = diagnose(&location, &FakeHost::macos());

    assert_eq!(status_of(&status, "status-item-config"), StatusValue::Error);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::BasePathNotDirectory)
    );
}
