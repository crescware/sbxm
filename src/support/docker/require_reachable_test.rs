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

#[test]
fn an_engine_that_answers_with_a_failure_keeps_what_it_said() -> Checked {
    let host = FakeHost::macos().failing(
        "docker version --format {{.Server.Version}}",
        "Cannot connect to the Docker daemon",
        1,
    );

    let error = require_reachable(&host).refused_because("the engine refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    let external = diagnostic
        .external
        .as_ref()
        .required_because("what docker said is kept")?;
    assert_eq!(external.stderr, b"Cannot connect to the Docker daemon");
    assert_eq!(external.exit_status, "exit status: 1");
    Ok(())
}

#[test]
fn an_interrupted_probe_is_not_reported_as_an_unreachable_engine() -> Checked {
    let host = crate::testing::host::Unrunnable::canceled(FakeHost::macos(), "version --format");

    let error = require_reachable(&host).refused_because("the user interrupted")?;

    assert!(
        matches!(error, crate::diagnostics::Error::Canceled),
        "{error:?}"
    );
    Ok(())
}
