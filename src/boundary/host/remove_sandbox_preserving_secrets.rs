use std::path::Path;

use crate::boundary::host::protocol::{DaemonState, parse_daemon_status};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, unparseable};
use crate::msg;

/// `daemon API`で削除し、scoped secretの値に触れずに保持する。
/// 通常削除はforce=falseで、runtime自身のin-use検査を維持する。
/// 呼び出し元のinventoryは、通常削除ならProtectionPermitを消費済みである。
pub(crate) fn remove_sandbox_preserving_secrets(
    host: &dyn HostEnvironment,
    name: &str,
    force: bool,
) -> Result<()> {
    let spec = CommandSpec::capture("sbx", &["daemon", "status", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::Probe);
    let output = host.run(&spec)?.require_success()?.stdout_text();
    if parse_daemon_status(&output)? != DaemonState::Running {
        // 停止は読み取れた状態であり、出力の解釈の失敗ではない。sbxmはdaemonを起動しない。
        return Err(Error::single(
            Diagnostic::new(ErrorId::DaemonNotRunning, msg!("error-daemon-not-running"))
                .remediation(
                    crate::design::Remediation::text(msg!("remediation-daemon-not-running"))
                        .try_run("sbx daemon start"),
                ),
        ));
    }
    let document: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| unparseable("sbx daemon status", &error.to_string()))?;
    let socket = document
        .get("socket")
        .and_then(serde_json::Value::as_str)
        .filter(|value| Path::new(value).is_absolute() && !value.contains('\0'))
        .ok_or_else(|| {
            unparseable(
                "sbx daemon status",
                "the daemon has no absolute socket path",
            )
        })?;
    let url = format!("http://localhost/sandbox/{name}");
    let output = request(host, socket, "GET", &url)?;
    let document: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| unparseable("sandbox inspect", &error.to_string()))?;
    if document.get("name").and_then(serde_json::Value::as_str) != Some(name) {
        return Err(not_confirmed(name, "the sandbox name does not match"));
    }
    let id = document
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        })
        .ok_or_else(|| not_confirmed(name, "the sandbox has no valid runtime ID"))?;
    // expected_idは同名Sandboxへの入れ替わりを拒否する。secret保持を指定できなかった
    // 場合に通常のrmへfallbackしてはならない。
    let url = format!("{url}?force={force}&expected_id={id}&delete_scoped_secrets=false");
    request(host, socket, "DELETE", &url)?;
    Ok(())
}

fn request(host: &dyn HostEnvironment, socket: &str, method: &str, url: &str) -> Result<String> {
    // 対応hostのmacOSが供給するcurlを使う。HTTP接続先はdaemonが示したUnix socketのみ。
    let spec = CommandSpec::capture(
        "curl",
        &[
            "--disable",
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--noproxy",
            "*",
            "--unix-socket",
            socket,
            "--request",
            method,
            url,
        ],
    )
    .env(EnvPolicy::InheritWithoutSshAgent)
    .timeout(TimeoutClass::SandboxLifecycle);
    let mut outcome = host.run(&spec)?;
    if !outcome.success()
        && let Ok(document) = serde_json::from_slice::<serde_json::Value>(&outcome.stdout)
        && let Some(message) = document.get("message").and_then(serde_json::Value::as_str)
    {
        outcome.stderr.extend_from_slice(b"\n");
        outcome.stderr.extend_from_slice(message.as_bytes());
    }
    if !outcome.success() && super::protocol::is_login_missing(&outcome.stderr) {
        return Err(Error::single(
            Diagnostic::new(ErrorId::SbxLoginMissing, msg!("error-sbx-login-missing"))
                .remediation(
                    crate::design::Remediation::text(msg!("remediation-sbx-login"))
                        .try_run("sbx login"),
                )
                .external(outcome.failure()),
        ));
    }
    Ok(outcome.require_success()?.stdout_text())
}

fn not_confirmed(name: &str, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandNotConfirmed,
            msg!("error-external-command-not-confirmed", subject = name),
        )
        .fact(Fact::cause(cause))
        .remediation(msg!("remediation-external-command-not-confirmed")),
    )
}

#[cfg(test)]
#[path = "remove_sandbox_preserving_secrets_test.rs"]
mod remove_sandbox_preserving_secrets_test;
