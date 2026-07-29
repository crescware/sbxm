use super::*;
use crate::error::ErrorId;

#[test]
fn the_daemon_status_parser_reads_the_status_line_of_the_real_output() {
    // 対象versionが実際に出力する形。socketとlogのpathは読まない。
    let observed = "Status: running\nSocket: /Users/<user>/Library/Application Support/com.docker.sandboxes/sandboxes/sandboxd/sandboxd.sock\nLogs: /Users/<user>/Library/Application Support/com.docker.sandboxes/sandboxes/sandboxd/daemon.log\n";
    assert_eq!(parse_daemon_status(observed).unwrap(), DaemonState::Running);

    assert_eq!(
        parse_daemon_status("Status: stopped\n").unwrap(),
        DaemonState::Stopped
    );
    assert_eq!(
        parse_daemon_status("Status: Running\n").unwrap(),
        DaemonState::Running
    );

    for output in [
        "",
        "Socket: /tmp/sandboxd.sock\n",
        "Status: degraded\n",
        r#"{"running": true}"#,
    ] {
        let error = parse_daemon_status(output).expect_err("unknown states are not guessed");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}
