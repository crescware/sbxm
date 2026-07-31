use crate::diagnostics::Result;

use crate::compatibility::json::unparseable;

use super::DaemonState;

/// `sbx daemon status`をparseする。
pub fn parse_daemon_status(output: &str) -> Result<DaemonState> {
    // `sbx daemon status`はJSONを持たず、`<label>: <value>`の行を並べる。
    // socketとlogのpathはhostのuser名を含むため読まない。
    let state = output.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(label, _)| label.trim().eq_ignore_ascii_case("status"))
            .map(|(_, value)| value.trim().to_ascii_lowercase())
    });

    match state.as_deref() {
        Some("running") => Ok(DaemonState::Running),
        Some("stopped" | "not running" | "not-running") => Ok(DaemonState::Stopped),
        Some(other) => Err(unparseable(
            "sbx daemon status",
            &format!("status {other} has no defined meaning in this build"),
        )),
        None => Err(unparseable(
            "sbx daemon status",
            "no line states the daemon status",
        )),
    }
}
