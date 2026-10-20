use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn the_daemon_status_is_read_from_v048_json() -> Checked {
    for (output, expected) in [
        (
            r#"{"status":"running","socket":"/tmp/sandboxd.sock","logs":"/tmp/daemon.log"}"#,
            DaemonState::Running,
        ),
        (
            r#"{"status":"stopped","socket":"/tmp/sandboxd.sock"}"#,
            DaemonState::Stopped,
        ),
    ] {
        assert_eq!(parse_daemon_status(output).required()?, expected);
    }
    for output in [
        "",
        "Status: running\n",
        "{}",
        "true",
        r#"{"status":"degraded"}"#,
        r#"{"status":true}"#,
    ] {
        let error = parse_daemon_status(output).refused()?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}
