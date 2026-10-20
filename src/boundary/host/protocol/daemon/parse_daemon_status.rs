use crate::diagnostics::{Result, unparseable};

use super::DaemonState;

/// `sbx daemon status --json`をparseする。socketやlogのpathは読まない。
pub fn parse_daemon_status(output: &str) -> Result<DaemonState> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx daemon status", &error.to_string()))?;
    match document.get("status").and_then(serde_json::Value::as_str) {
        Some("running") => Ok(DaemonState::Running),
        Some("stopped") => Ok(DaemonState::Stopped),
        Some(other) => Err(unparseable(
            "sbx daemon status",
            &format!("status {other} has no defined meaning in this build"),
        )),
        None => Err(unparseable(
            "sbx daemon status",
            "the document has no status",
        )),
    }
}
