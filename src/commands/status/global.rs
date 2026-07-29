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

use crate::support::{Row, StatusValue};

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
fn check_network_policy(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["policy", "ls"]) {
        Ok(output) => match parse_network_policy(&output) {
            Ok(observed) if observed == EXPECTED_NETWORK_POLICY => {
                push(status, "status-item-network-policy", StatusValue::Ready);
            }
            Ok(observed) => {
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
#[path = "global_test.rs"]
mod global_test;
