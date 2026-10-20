use crate::boundary::host::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::diagnostics::ErrorId;
use crate::testing::host::{FakeSbx, removal_command};
use crate::testing::outcome::{Checked, Refused, Required};

use super::remove_sandbox_preserving_secrets;

const NAME: &str = "sbxm-example";
const INSPECT: &str = "--disable --silent --show-error --fail-with-body --noproxy * --unix-socket /tmp/sbxm-test-sandboxd.sock --request GET http://localhost/sandbox/sbxm-example";

#[test]
fn deletion_keeps_secrets_pins_the_runtime_and_preserves_the_force_choice() -> Checked {
    for force in [false, true] {
        let host = FakeSbx::listing("[]");
        remove_sandbox_preserving_secrets(&host, NAME, force).required()?;
        let calls = host.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], ["daemon", "status", "--json"]);
        assert_eq!(calls[1].join(" "), INSPECT);
        assert_eq!(calls[2].join(" "), removal_command(NAME, force));
        for spec in host.specs.borrow().iter() {
            assert_eq!(spec.env, EnvPolicy::InheritWithoutSshAgent);
            assert_eq!(spec.output(), OutputPolicy::Capture);
        }
        assert_eq!(
            host.spec("--request DELETE")?.timeout,
            TimeoutClass::SandboxLifecycle
        );
        assert!(
            !calls
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "secret"))
        );
        assert!(
            !calls
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "rm"))
        );
    }
    Ok(())
}

#[test]
fn an_unobservable_daemon_or_socket_stops_before_any_api_request() -> Checked {
    for output in [
        "not json",
        r#"{"status":"running"}"#,
        r#"{"status":"running","socket":"relative"}"#,
        r#"{"status":"running","socket":"/tmp/\u0000socket"}"#,
    ] {
        let host = FakeSbx::listing("[]").answering("daemon status --json", 0, output);
        assert_eq!(
            remove_sandbox_preserving_secrets(&host, NAME, false)
                .refused()?
                .first_id(),
            Some(ErrorId::ExternalOutputUnparseable)
        );
        assert_eq!(host.calls().len(), 1);
    }
    Ok(())
}

#[test]
fn a_stopped_daemon_is_reported_as_not_running_before_any_api_request() -> Checked {
    // 停止は読み取れた状態である。出力を解釈できなかったこととは分けて示す。
    let host = FakeSbx::listing("[]").answering(
        "daemon status --json",
        0,
        r#"{"status":"stopped","socket":"/tmp/socket"}"#,
    );
    let error = remove_sandbox_preserving_secrets(&host, NAME, false).refused()?;
    assert_eq!(error.first_id(), Some(ErrorId::DaemonNotRunning));
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.description.id, "error-daemon-not-running");
    let remediation = diagnostic.remediation.as_ref().required()?;
    assert_eq!(
        remediation.explanation[0].id,
        "remediation-daemon-not-running"
    );
    assert_eq!(remediation.commands[0].as_str(), "sbx daemon start");
    assert_eq!(host.calls().len(), 1);
    Ok(())
}

#[test]
fn a_sandbox_that_cannot_be_pinned_points_at_the_sandboxes_that_exist_now() -> Checked {
    // 名前やruntime IDを確かめられないのは、作業中に置き換わったか消えた場合である。
    // 下に何も示さないため、出力を見るよう求める案内は使わない。
    let host = FakeSbx::listing("[]").answering(INSPECT, 0, r#"{"name":"other","id":"a1"}"#);
    let error = remove_sandbox_preserving_secrets(&host, NAME, false).refused()?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::ExternalCommandNotConfirmed);
    assert!(diagnostic.external.is_none());
    let remediation = diagnostic.remediation.as_ref().required()?;
    assert_eq!(
        remediation.explanation[0].id,
        "remediation-external-command-not-confirmed"
    );
    Ok(())
}

#[test]
fn an_unobservable_or_different_sandbox_is_never_deleted() -> Checked {
    for output in [
        "not json",
        "{}",
        r#"{"name":"other","id":"a1"}"#,
        r#"{"name":"sbxm-example"}"#,
        r#"{"name":"sbxm-example","id":""}"#,
        r#"{"name":"sbxm-example","id":"a1&delete_scoped_secrets=true"}"#,
    ] {
        let host = FakeSbx::listing("[]").answering(INSPECT, 0, output);
        assert_eq!(
            remove_sandbox_preserving_secrets(&host, NAME, false)
                .refused()?
                .first_id(),
            Some(if output == "not json" {
                ErrorId::ExternalOutputUnparseable
            } else {
                ErrorId::ExternalCommandNotConfirmed
            })
        );
        assert!(!host.removed_any());
    }
    Ok(())
}

#[test]
fn api_failures_and_in_use_refusals_never_fall_back_to_cli_removal() -> Checked {
    for command in [
        "daemon status --json".to_string(),
        INSPECT.to_string(),
        removal_command(NAME, false),
    ] {
        let host =
            FakeSbx::listing("[]").answering(&command, 22, r#"{"message":"sandbox is in use"}"#);
        let error = remove_sandbox_preserving_secrets(&host, NAME, false).refused()?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
        if command != "daemon status --json" {
            let external = error
                .diagnostics()
                .first()
                .required()?
                .external
                .as_ref()
                .required()?;
            assert!(String::from_utf8_lossy(&external.stderr).contains("sandbox is in use"));
        }
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "rm"))
        );
    }
    Ok(())
}

#[test]
fn authentication_failures_keep_the_sign_in_diagnostic() -> Checked {
    let host =
        FakeSbx::listing("[]").answering(INSPECT, 22, r#"{"message":"not signed in to Docker"}"#);
    let error = remove_sandbox_preserving_secrets(&host, NAME, false).refused()?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxLoginMissing));
    assert!(!host.removed_any());
    Ok(())
}

#[test]
fn an_http_failure_without_a_json_message_is_still_a_failure() -> Checked {
    for output in ["not JSON", "{}"] {
        let host = FakeSbx::listing("[]").answering(INSPECT, 22, output);
        assert_eq!(
            remove_sandbox_preserving_secrets(&host, NAME, false)
                .refused()?
                .first_id(),
            Some(ErrorId::ExternalCommandFailed)
        );
        assert!(!host.removed_any());
    }
    Ok(())
}
