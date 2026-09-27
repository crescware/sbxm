use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::add_request::from;
use crate::testing::project::ssh_repository;

fn asking_for(worktrees: Option<u32>) -> Checked<AddRequest> {
    Ok(from(
        ssh_repository("example-org/example-repo")?,
        worktrees,
        Some("develop"),
    ))
}

#[test]
fn a_worktree_count_outside_the_allowed_range_is_refused_with_the_bounds() -> Checked {
    for value in [0, MAX_WORKTREES + 1, u32::MAX] {
        let error = TargetConfiguration::from_request(&asking_for(Some(value))?)
            .refused_because("{value} worktrees are outside the allowed range")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreesOutOfRange),
            "{value} produced the wrong error"
        );

        // 拒否した値と、受け付ける範囲の両端を示す。読み手に上限を暗記させない。
        let description = &error
            .diagnostics()
            .first()
            .required_because("the refusal carries a diagnostic")?
            .description;
        assert!(
            description.args.contains(&("value", value.to_string()))
                && description
                    .args
                    .contains(&("minimum", MIN_WORKTREES.to_string()))
                && description
                    .args
                    .contains(&("maximum", MAX_WORKTREES.to_string())),
            "the refusal names the value and the range: {:?}",
            description.args
        );
    }
    Ok(())
}

#[test]
fn both_ends_of_the_allowed_range_are_inside_it() -> Checked {
    for value in [MIN_WORKTREES, MAX_WORKTREES] {
        let target = TargetConfiguration::from_request(&asking_for(Some(value))?)
            .required_because("{value} worktrees are within the allowed range")?;
        assert_eq!(target.requested_worktrees, value);
        assert_eq!(target.mode, CreationMode::Detached);
    }
    Ok(())
}

#[test]
fn the_branch_a_host_repository_is_on_is_checked_like_a_detach_branch() -> Checked {
    // `--local`の起点は、hostのrepositoryが今いるbranchである。利用者が打った値ではなくても、
    // 外部commandへ渡す前に同じ規則で確かめる。
    let local = crate::repository::RepositoryIdentity::local("/home/user/code/app/.git", "app")
        .required_because("a local repository")?;
    let mut request = from(local, None, None);
    request.start_branch = Some("-x".to_string());

    let error = TargetConfiguration::from_request(&request)
        .refused_because("a branch that reads as an option")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));

    request.start_branch = Some("main".to_string());
    let target = TargetConfiguration::from_request(&request).required()?;
    assert_eq!(target.mode, CreationMode::Attached);
    assert_eq!(target.start_ref.as_deref(), Some("main"));
    Ok(())
}
