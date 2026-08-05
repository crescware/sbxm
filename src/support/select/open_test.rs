use std::thread;
use std::time::Duration;

use crate::metadata;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::Fixture;

use super::super::candidates;
use super::MetadataMaximums;

#[test]
fn a_metadata_maximum_arrives_after_the_prompt_can_start() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    project.metadata.provisioning.requested_worktrees = 5;
    metadata::update(&project.paths, &project.metadata)
        .required_because("record the calculated worktree count")?;

    let candidates = candidates(&fixture.location).required_because("load candidates")?;
    let mut maximums = MetadataMaximums::new(&candidates);

    assert_eq!(
        maximums.poll(0),
        None,
        "the initial poll starts without waiting"
    );
    for _ in 0..100 {
        if maximums.poll(0) == Some(4) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(1));
    }
    Err(crate::testing::outcome::Unmet::new(
        "the background metadata calculation did not arrive",
    ))
}

#[test]
fn a_result_for_an_unknown_project_is_ignored() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let candidates = candidates(&fixture.location).required_because("load candidates")?;
    let mut maximums = MetadataMaximums::new(&candidates);

    let _ = maximums.sender.send((99, Some(4)));
    assert_eq!(maximums.poll(99), None);
    Ok(())
}
