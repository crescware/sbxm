use std::thread;
use std::time::{Duration, Instant};

use crate::metadata;
use crate::testing::outcome::{Checked, Required, Unmet};
use crate::testing::project::Fixture;

use super::super::candidates;
use super::MetadataMaximums;

/// 到着そのものを確かめる。負荷のかかったmachineでも打ち切らないよう、
/// 回数ではなく経過時間で待つ。
fn wait_for(maximums: &mut MetadataMaximums, project: usize, expected: u32) -> Checked {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if maximums.poll(project) == Some(expected) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err(Unmet::new(
        "the background metadata calculation did not arrive",
    ))
}

#[test]
fn every_project_is_calculated_without_waiting_for_the_cursor() -> Checked {
    let fixture = Fixture::new()?;
    for (id, worktrees) in [("example-org/example-repo", 5), ("other/other-repo", 3)] {
        let mut project = fixture.register(id)?;
        project.metadata.provisioning.requested_worktrees = worktrees;
        metadata::update(&project.paths, &project.metadata)
            .required_because("record the calculated worktree count")?;
    }

    let candidates = candidates(&fixture.location).required_because("load candidates")?;
    assert_eq!(
        candidates
            .iter()
            .map(super::Candidate::display_id)
            .collect::<Vec<String>>(),
        vec![
            "example-org/example-repo".to_string(),
            "other/other-repo".to_string()
        ],
        "the order the prompt shows decides which index is which"
    );

    let mut maximums = MetadataMaximums::new(&candidates);

    // カーソルは先頭に当たったままにする。2件目はpollで要求していない。
    wait_for(&mut maximums, 0, 4)?;
    wait_for(&mut maximums, 1, 2)?;
    Ok(())
}

#[test]
fn a_project_outside_the_list_has_no_maximum() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let candidates = candidates(&fixture.location).required_because("load candidates")?;
    let mut maximums = MetadataMaximums::new(&candidates);

    assert_eq!(
        maximums.poll(99),
        None,
        "an index the registry cannot answer for is not read out of another project's slot"
    );
    Ok(())
}
