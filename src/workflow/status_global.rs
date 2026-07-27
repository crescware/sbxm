//! `sbxm status --global`。
//!
//! hostとglobal環境をread-onlyで診断する。login、setup、file更新、daemon起動・停止を
//! 行わない。問題がある場合は、利用者が直接実行する外部commandを表示する。
//!
//! 検査対象は、sbxm自身がhost上で直接使用する設定、platform、command、serviceに限定する。

use std::path::Path;

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{
    CliVersion, EXPECTED_NETWORK_POLICY, parse_daemon_status, parse_login_status,
    parse_network_policy, require_minimum_version,
};
use crate::config::{self, ConfigLocation, ConfigState};
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths;

use super::{Row, StatusValue};

/// 期待するplatform。翻訳しない技術表記。
const EXPECTED_PLATFORM: &str = "macOS >= 14 on arm64";
const MINIMUM_MACOS_MAJOR: u32 = 14;
const EXPECTED_ARCHITECTURE: &str = "arm64";

/// hostが直接使用するcommand。
const REQUIRED_COMMANDS: [&str; 4] = ["git", "ssh", "docker", "sbx"];

/// 診断結果。
pub struct GlobalStatus {
    pub rows: Vec<Row>,
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<Msg>,
}

impl GlobalStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// hostとglobal環境を診断する。何も変更しない。
pub fn diagnose(location: &ConfigLocation, host: &dyn HostEnvironment) -> GlobalStatus {
    let mut status = GlobalStatus {
        rows: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. global configとbase path
    let config = check_config(location, &mut status);
    check_base_path(config.as_deref(), &mut status);

    // 2. platform
    check_platform(host, &mut status);

    // 3-4. hostが直接実行するcommandと、Docker Client/Server疎通
    let present = check_host_commands(host, &mut status);

    // 5-9. Docker Sandboxes CLIとそのserviceの状態
    check_docker_sandboxes(host, present.contains(&"sbx"), &mut status);

    status
}

fn push(status: &mut GlobalStatus, item: &'static str, value: StatusValue) {
    status.rows.push(Row {
        item,
        status: value,
    });
}

fn check_config(
    location: &ConfigLocation,
    status: &mut GlobalStatus,
) -> Option<Box<config::GlobalConfig>> {
    match config::load(location) {
        Ok(ConfigState::Valid { config, warnings }) => {
            push(status, "status-item-config", StatusValue::Ready);
            status.warnings.extend(warnings);
            Some(config)
        }
        Ok(ConfigState::Missing) => {
            push(status, "status-item-config", StatusValue::Missing);
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::ConfigMissing,
                    msg!(
                        "error-config-missing",
                        path = paths::display(&location.config_file())
                    ),
                )
                .remediation(msg!("remediation-run-init")),
            );
            None
        }
        Err(error) => {
            push(status, "status-item-config", StatusValue::Error);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

fn check_base_path(config: Option<&config::GlobalConfig>, status: &mut GlobalStatus) {
    let Some(config) = config else {
        // configを読めない場合、base pathは宣言自体が存在しない。
        push(status, "status-item-base-path", StatusValue::Missing);
        return;
    };

    let path = config.base_path.as_path();
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            if is_writable_dir(path) {
                push(status, "status-item-base-path", StatusValue::Ready);
            } else {
                push(status, "status-item-base-path", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::BasePathNotWritable,
                    msg!("error-base-path-not-writable", path = paths::display(path)),
                ));
            }
        }
        Ok(_) => {
            push(status, "status-item-base-path", StatusValue::Error);
            status.diagnostics.push(Diagnostic::new(
                ErrorId::BasePathNotDirectory,
                msg!("error-base-path-not-directory", path = paths::display(path)),
            ));
        }
        Err(_) => {
            // `add`が作成するため、未作成であること自体はerrorではない。
            push(status, "status-item-base-path", StatusValue::Missing);
        }
    }
}

fn is_writable_dir(path: &Path) -> bool {
    let probe = path.join(".sbxm-write-probe");
    match std::fs::create_dir(&probe) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&probe);
            true
        }
        Err(error) => error.kind() == std::io::ErrorKind::AlreadyExists,
    }
}

fn check_platform(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    let version = read_stdout(host, "sw_vers", &["-productVersion"]);
    let architecture = read_stdout(host, "uname", &["-m"]);

    match (version, architecture) {
        (Ok(version), Ok(architecture)) => {
            let version = version.trim().to_string();
            let architecture = architecture.trim().to_string();
            let major = version
                .split('.')
                .next()
                .and_then(|value| value.parse::<u32>().ok());
            let observed = format!("macOS {version} on {architecture}");
            match major {
                Some(major)
                    if major >= MINIMUM_MACOS_MAJOR && architecture == EXPECTED_ARCHITECTURE =>
                {
                    push(status, "status-item-platform", StatusValue::Ready);
                }
                Some(_) => {
                    push(status, "status-item-platform", StatusValue::Error);
                    status.diagnostics.push(Diagnostic::new(
                        ErrorId::PlatformUnsupported,
                        msg!(
                            "error-platform-unsupported",
                            expected = EXPECTED_PLATFORM,
                            observed = observed
                        ),
                    ));
                }
                None => {
                    push(status, "status-item-platform", StatusValue::Error);
                    status.diagnostics.push(Diagnostic::new(
                        ErrorId::PlatformUnobservable,
                        msg!(
                            "error-platform-unobservable",
                            detail = format!("the macOS version {version} could not be read")
                        ),
                    ));
                }
            }
        }
        (Err(error), _) | (_, Err(error)) => {
            // 観測できない場合に推測した状態を返さない。
            push(status, "status-item-platform", StatusValue::Error);
            status.diagnostics.push(Diagnostic::new(
                ErrorId::PlatformUnobservable,
                msg!("error-platform-unobservable", detail = describe(&error)),
            ));
        }
    }
}

/// hostが直接実行するcommandの存在と、Docker Client/Server疎通。
fn check_host_commands(host: &dyn HostEnvironment, status: &mut GlobalStatus) -> Vec<&'static str> {
    let mut present = Vec::new();
    for program in REQUIRED_COMMANDS {
        let exists = host.command_exists(program);
        if exists {
            present.push(program);
        }
        // Dockerとsbxは、存在確認より後の検査結果とまとめて1行にする。
        if program == "docker" || program == "sbx" {
            continue;
        }
        let item = match program {
            "git" => "status-item-git",
            "ssh" => "status-item-ssh",
            _ => unreachable!("unexpected required command {program}"),
        };
        if exists {
            push(status, item, StatusValue::Ready);
        } else {
            push(status, item, StatusValue::Missing);
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::HostCommandMissing,
                    msg!("error-host-command-missing", command = program),
                )
                .remediation(msg!("remediation-install-command", command = program)),
            );
        }
    }

    if !present.contains(&"docker") {
        push(status, "status-item-docker", StatusValue::Missing);
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", command = "docker"),
            )
            .remediation(msg!("remediation-install-command", command = "docker")),
        );
        return present;
    }

    match read_stdout(
        host,
        "docker",
        &["version", "--format", "{{.Server.Version}}"],
    ) {
        Ok(output) if !output.trim().is_empty() => {
            push(status, "status-item-docker", StatusValue::Ready);
        }
        Ok(_) => {
            push(status, "status-item-docker", StatusValue::Error);
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::DockerUnreachable,
                    msg!(
                        "error-docker-unreachable",
                        detail = "the server version was empty"
                    ),
                )
                .remediation(msg!("remediation-start-docker")),
            );
        }
        Err(error) => {
            push(status, "status-item-docker", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::DockerUnreachable,
                msg!("error-docker-unreachable", detail = describe(&error)),
            )
            .remediation(msg!("remediation-start-docker"));
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }

    present
}

/// Docker Sandboxes CLIのversion、network policy、daemonの状態。
fn check_docker_sandboxes(
    host: &dyn HostEnvironment,
    sbx_present: bool,
    status: &mut GlobalStatus,
) {
    let dependent_items = [
        "status-item-network-policy",
        "status-item-daemon",
        "status-item-login",
        "status-item-session-inspection",
        "status-item-remote-ssh",
    ];

    if !sbx_present {
        push(status, "status-item-docker-sandboxes", StatusValue::Missing);
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", command = "sbx"),
            )
            .remediation(msg!("remediation-install-command", command = "sbx")),
        );
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    }

    let output = match read_stdout(host, "sbx", &["version"]) {
        Ok(output) => output,
        Err(error) => {
            push(status, "status-item-docker-sandboxes", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::SbxVersionUnparseable,
                msg!("error-sbx-version-unparseable", observed = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
            for item in dependent_items {
                push(status, item, StatusValue::Error);
            }
            return;
        }
    };

    let Some(observed) = CliVersion::extract_from_output(&output) else {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status.diagnostics.push(Diagnostic::new(
            ErrorId::SbxVersionUnparseable,
            msg!("error-sbx-version-unparseable", observed = output.trim()),
        ));
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    };

    if let Err(error) = require_minimum_version(observed) {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status
            .diagnostics
            .extend(error.diagnostics().iter().cloned());
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    }

    push(status, "status-item-docker-sandboxes", StatusValue::Ready);
    check_network_policy(host, status);
    check_daemon(host, status);
    check_login(host, status);
    check_session_inspection(host, status);
    check_remote_ssh(host, status);
}

/// Remote SSHでSandboxへ接続できる設定になっているか。
///
/// `open`は`<sandbox-name>.sbx`へsshするため、その名前をsshが解決できることを
/// read-onlyで確かめる。設定方法は対象versionごとに異なるため、実機で確認していない
/// setup commandは案内しない。
fn check_remote_ssh(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    // `ssh -G`は接続せず、その宛先に対する実効設定だけを表示する。
    match read_stdout(host, "ssh", &["-G", "sbxm-probe.sbx"]) {
        Ok(output) => {
            let configured = output
                .lines()
                .any(|line| line.trim().to_ascii_lowercase().starts_with("proxycommand"));
            if configured {
                push(status, "status-item-remote-ssh", StatusValue::Ready);
            } else {
                push(status, "status-item-remote-ssh", StatusValue::Missing);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::RemoteSshUnconfigured,
                        msg!("error-remote-ssh-unconfigured", host = "*.sbx"),
                    )
                    .remediation(msg!("remediation-remote-ssh-unconfigured")),
                );
            }
        }
        Err(error) => {
            push(status, "status-item-remote-ssh", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::RemoteSshUnobservable,
                msg!("error-remote-ssh-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

/// Docker Sandboxesへのlogin状態。
///
/// loginを前提とするのはTemplateとSandboxを扱う工程であり、observeできない場合に
/// login済みと推測しない。
fn check_login(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["login", "status", "--json"]) {
        Ok(output) => match parse_login_status(&output) {
            Ok(true) => push(status, "status-item-login", StatusValue::Ready),
            Ok(false) => {
                push(status, "status-item-login", StatusValue::Missing);
                status.diagnostics.push(
                    Diagnostic::new(ErrorId::SbxLoginMissing, msg!("error-sbx-login-missing"))
                        .remediation(msg!("remediation-sbx-login", command = "sbx login")),
                );
            }
            Err(error) => {
                push(status, "status-item-login", StatusValue::Error);
                status
                    .diagnostics
                    .extend(error.diagnostics().iter().cloned());
            }
        },
        Err(error) => {
            push(status, "status-item-login", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::SbxLoginUnobservable,
                msg!("error-sbx-login-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

/// active session検査の対応状況。
///
/// daemonを安全に再起動できるかどうかは、この検査ができるかどうかで決まる。
/// Sandboxが1件もない場合は、sessionを持ち得る対象がないため対応済みとして扱う。
fn check_session_inspection(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["ls", "--json"]) {
        Ok(output) => match crate::compatibility::parse_sandbox_list(&output) {
            Ok(sandboxes) => {
                let hidden: Vec<String> = sandboxes
                    .iter()
                    .filter(|entry| entry.active_sessions.is_none())
                    .map(|entry| entry.name.clone())
                    .collect();
                if hidden.is_empty() {
                    push(status, "status-item-session-inspection", StatusValue::Ready);
                } else {
                    push(status, "status-item-session-inspection", StatusValue::Error);
                    status.diagnostics.push(
                        Diagnostic::new(
                            ErrorId::DaemonSessionUnobservable,
                            msg!(
                                "error-daemon-session-unobservable",
                                sandbox = hidden.join(", ")
                            ),
                        )
                        .remediation(msg!("remediation-daemon-session-unobservable")),
                    );
                }
            }
            Err(error) => {
                push(status, "status-item-session-inspection", StatusValue::Error);
                status
                    .diagnostics
                    .extend(error.diagnostics().iter().cloned());
            }
        },
        Err(error) => {
            push(status, "status-item-session-inspection", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::DaemonSessionUnobservable,
                msg!(
                    "error-daemon-session-unobservable",
                    sandbox = describe(&error)
                ),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

fn check_network_policy(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["policy", "ls"]) {
        Ok(output) => match parse_network_policy(&output) {
            Ok(observed) if observed == EXPECTED_NETWORK_POLICY => {
                push(status, "status-item-network-policy", StatusValue::Ready);
            }
            Ok(observed) => {
                // より制限が強いpolicyも動作と安全性を推測して受け入れない。
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::NetworkPolicyMismatch,
                        msg!(
                            "error-network-policy-mismatch",
                            observed = observed,
                            expected = EXPECTED_NETWORK_POLICY
                        ),
                    )
                    .remediation(msg!(
                        "remediation-network-policy",
                        expected = EXPECTED_NETWORK_POLICY
                    )),
                );
            }
            Err(error) => {
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::NetworkPolicyUnobservable,
                    msg!(
                        "error-network-policy-unobservable",
                        detail = describe(&error)
                    ),
                ));
            }
        },
        Err(error) => {
            push(status, "status-item-network-policy", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::NetworkPolicyUnobservable,
                msg!(
                    "error-network-policy-unobservable",
                    detail = describe(&error)
                ),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

fn check_daemon(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["daemon", "status"]) {
        Ok(output) => match parse_daemon_status(&output) {
            Ok(state) => {
                let value = match state {
                    crate::compatibility::DaemonState::Running => StatusValue::Running,
                    crate::compatibility::DaemonState::Stopped => StatusValue::Stopped,
                };
                push(status, "status-item-daemon", value);
            }
            Err(error) => {
                push(status, "status-item-daemon", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::DaemonUnobservable,
                    msg!("error-daemon-unobservable", detail = describe(&error)),
                ));
            }
        },
        Err(error) => {
            push(status, "status-item-daemon", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::DaemonUnobservable,
                msg!("error-daemon-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

fn read_stdout(host: &dyn HostEnvironment, program: &str, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::probe(program, args)
        .env(EnvPolicy::Inherit)
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec)?;
    let outcome = outcome.require_success()?;
    Ok(outcome.stdout_text())
}

fn describe(error: &Error) -> String {
    error
        .diagnostics()
        .first()
        .map(|diagnostic| diagnostic.id.to_string())
        .unwrap_or_else(|| "canceled".to_string())
}

fn external_of(error: &Error) -> Option<crate::error::ExternalFailure> {
    error
        .diagnostics()
        .first()
        .and_then(|diagnostic| diagnostic.external.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use crate::i18n::{Catalog, Locale};
    use crate::workflow::Reporter;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;

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
                Some(Ok((stdout, stderr, code))) => Ok(CommandOutcome {
                    program: spec.program.clone(),
                    args: spec.args.clone(),
                    working_dir: spec.working_dir.clone(),
                    status: std::process::ExitStatus::from_raw(code << 8),
                    stdout: stdout.clone().into_bytes(),
                    stderr: stderr.clone().into_bytes(),
                    stderr_lossy: false,
                }),
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
            std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700))
                .unwrap();
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
                "status-item-session-inspection",
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
            .responding("sbx daemon status", r#"{"running": true}"#);
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
}
