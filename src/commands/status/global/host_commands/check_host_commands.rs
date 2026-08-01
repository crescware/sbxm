use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

use super::REQUIRED_COMMANDS;

/// hostが直接実行するcommandの存在と、Docker Client/Server疎通。
pub fn check_host_commands(
    host: &dyn HostEnvironment,
    status: &mut GlobalStatus,
) -> Vec<&'static str> {
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
                    msg!("error-host-command-missing", program = program),
                )
                .remediation(msg!("remediation-install-command", program = program)),
            );
        }
    }

    if !present.contains(&"docker") {
        push(status, "status-item-docker", StatusValue::Missing);
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", program = "docker"),
            )
            .remediation(msg!("remediation-install-command", program = "docker")),
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
                Diagnostic::new(ErrorId::DockerUnreachable, msg!("error-docker-unreachable"))
                    .fact(Fact::reason(msg!("cause-server-version-empty")))
                    .remediation(msg!("remediation-start-docker")),
            );
        }
        Err(error) => {
            push(status, "status-item-docker", StatusValue::Error);
            let mut diagnostic =
                Diagnostic::new(ErrorId::DockerUnreachable, msg!("error-docker-unreachable"))
                    .fact(Fact::cause(&describe(&error)))
                    .remediation(msg!("remediation-start-docker"));
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }

    present
}
