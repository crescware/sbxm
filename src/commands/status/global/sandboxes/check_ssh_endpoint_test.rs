use crate::commands::status::global::GlobalStatus;
use crate::diagnostics::ErrorId;
use crate::support::StatusValue;
use crate::testing::host::{FakeSbx, SSH_FEATURE_ENABLED};

use super::check_ssh_endpoint;

fn status() -> GlobalStatus {
    GlobalStatus {
        rows: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    }
}

#[test]
fn a_stopped_or_unobservable_daemon_is_not_started_to_read_settings() {
    let host = FakeSbx::listing("[]");
    for daemon in [StatusValue::Stopped, StatusValue::Error] {
        let mut result = status();
        check_ssh_endpoint(&host, daemon, &mut result);
        assert_eq!(result.rows[0].status, daemon);
        assert!(host.calls().is_empty());
    }
}

#[test]
fn the_global_endpoint_setting_does_not_claim_to_be_a_live_connection_test() {
    let host = FakeSbx::listing("[]");
    let mut result = status();
    check_ssh_endpoint(&host, StatusValue::Running, &mut result);
    assert_eq!(result.rows[0].status, StatusValue::Ready);
    assert!(
        host.calls()
            .iter()
            .all(|args| args.first().is_some_and(|arg| arg == "settings"))
    );
    assert_eq!(
        host.specs.borrow()[0].env,
        crate::boundary::host::EnvPolicy::SbxSsh
    );
}

#[test]
fn disabled_restricted_and_unreadable_settings_have_distinct_diagnostics() {
    for (key, code, output, expected, value) in [
        (
            "settings get feature.ssh --json",
            0,
            SSH_FEATURE_ENABLED.replace("\"enabled\":true", "\"enabled\":false"),
            ErrorId::RemoteSshDisabled,
            StatusValue::Missing,
        ),
        (
            "settings get platform.allowExperimentalFeatures --json",
            0,
            r#"{"key":"platform.allowExperimentalFeatures","value":false}"#.to_string(),
            ErrorId::RemoteSshRestricted,
            StatusValue::Missing,
        ),
        (
            "settings get feature.ssh --json",
            0,
            "{}".to_string(),
            ErrorId::RemoteSshSettingUnobservable,
            StatusValue::Error,
        ),
    ] {
        let host = FakeSbx::listing("[]").answering(key, code, &output);
        let mut result = status();
        check_ssh_endpoint(&host, StatusValue::Running, &mut result);
        assert_eq!(result.rows[0].status, value);
        assert_eq!(result.diagnostics[0].id, expected);
    }
}
