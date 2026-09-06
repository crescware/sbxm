use crate::boundary::host::protocol::SandboxState;
use crate::diagnostics::ErrorId;
use crate::testing::metadata::attached;
use crate::testing::outcome::Checked;

use super::super::{Observation, ProvisioningState};
use super::require_observable;

#[test]
fn a_stopped_sandbox_is_named_with_the_command_that_starts_it() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observation = Observation::new(
        ProvisioningState::Unobservable,
        "current".to_string(),
        "stored".to_string(),
        "target".to_string(),
    );
    observation.sandbox_state = Some(SandboxState::Stopped);

    let error = require_observable(&metadata, &observation);
    assert!(error.contains_id(ErrorId::InitialProvisioningUnobservable));
    let rendered = format!("{:?}", error.diagnostics());
    assert!(rendered.contains("sbxm open"), "{rendered}");
    Ok(())
}

#[test]
fn a_state_that_was_never_observed_is_not_reported_as_a_known_one() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let observation = Observation::new(
        ProvisioningState::Unobservable,
        "current".to_string(),
        "stored".to_string(),
        "target".to_string(),
    );

    let error = require_observable(&metadata, &observation);
    let rendered = format!("{:?}", error.diagnostics());
    assert!(rendered.contains("unknown"), "{rendered}");
    Ok(())
}
