use crate::diagnostics::ErrorId;
use crate::testing::global_status::FakeHost;
use crate::testing::outcome::{Checked, Refused, Required};

use super::require_reachable;

#[test]
fn a_probe_timeout_is_reported_as_an_unreachable_docker_engine() -> Checked {
    let host = FakeHost::macos().timing_out("docker version --format {{.Server.Version}}");

    let error = require_reachable(&host)
        .refused_because("a daemon that does not answer is reported as unreachable")?;
    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert!(
        diagnostic.remediation.is_some(),
        "the timeout still tells the user how to restore Docker"
    );
    assert!(
        diagnostic.facts.iter().any(|fact| {
            matches!(fact, crate::design::Fact::OneLine { value, .. }
                if value.as_str() == "external-command-timeout")
        }),
        "the original probe failure remains visible: {:?}",
        diagnostic.facts
    );
    Ok(())
}
