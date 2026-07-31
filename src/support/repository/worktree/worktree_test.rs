use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::metadata;
use crate::metadata::CreationMode;
use crate::testing::repository::*;
use crate::testing::value::MOVED;

#[test]
fn a_project_that_asks_for_more_worktrees_gets_the_missing_ones_and_keeps_the_rest() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let existing = layout()?.worktree(0);
    // 既にあるtree-0はcommitを重ねて起点から離れている。増設のためにこれを作り直す
    // ことも、離れていることを理由に止まることもあってはならない。
    let host = worktree_host(CreationMode::Detached, 3)?
        .holding(&[&existing])
        .answering(&format!("git -C {existing} rev-parse HEAD"), MOVED);

    let project = metadata(CreationMode::Detached, Some("develop"), 3)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let managed = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .required_because("the worktrees that are missing are the ones that get made")?;

    assert_eq!(managed.len(), 3);
    assert!(
        !host.ran(&format!("worktree add --detach {existing}")),
        "the worktree that is already there is kept, not remade: {:?}",
        host.calls()
    );
    for index in 1..3 {
        assert!(
            host.ran(&format!(
                "worktree add --detach {} refs/remotes/origin/develop",
                layout()?.worktree(index)
            )),
            "{:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn an_attached_project_keeps_its_branch_and_gets_detached_worktrees_beside_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let existing = layout()?.worktree(0);
    // tree-0はbranchを持ったまま。Gitは同じbranchを2つのworktreeへcheckoutさせない
    // ため、足す側はdetachedになる。案件全体を移す必要はない。
    let host = worktree_host(CreationMode::Detached, 3)?
        .holding(&[&existing])
        .answering(&format!("git -C {existing} rev-parse HEAD"), MOVED)
        .answering(
            &format!("git -C {existing} symbolic-ref -q HEAD"),
            "refs/heads/develop\n",
        );

    let project = metadata(CreationMode::Attached, Some("develop"), 3)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let managed = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .required_because("an attached worktree does not stop the others from being made")?;

    assert_eq!(managed.len(), 3);
    for index in 1..3 {
        assert!(
            host.ran(&format!(
                "worktree add --detach {} refs/remotes/origin/develop",
                layout()?.worktree(index)
            )),
            "{:?}",
            host.calls()
        );
    }
    assert!(
        !host.ran("worktree add --track"),
        "the branch is already checked out, so no second worktree takes it: {:?}",
        host.calls()
    );
    assert!(
        !host.ran(&format!("worktree add --detach {existing}")),
        "the attached worktree keeps its branch instead of being remade: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn detached_worktrees_are_created_from_one_commit_and_recorded_one_by_one() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let host = worktree_host(CreationMode::Detached, 3)?;
    let project = metadata(CreationMode::Detached, Some("develop"), 3)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let managed = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .required_because("create")?;

    assert_eq!(
        managed,
        vec![
            "example-repo.tree-0",
            "example-repo.tree-1",
            "example-repo.tree-2"
        ]
    );
    for index in 0..3 {
        assert!(
            host.ran(&format!(
                "worktree add --detach {} refs/remotes/origin/develop",
                layout()?.worktree(index)
            )),
            "{:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn an_attached_project_gets_one_tracking_branch() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let host = worktree_host(CreationMode::Attached, 1)?;
    let project = metadata(CreationMode::Attached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .required_because("create")?;

    assert!(
        host.ran(&format!(
            "worktree add --track -b develop {} refs/remotes/origin/develop",
            layout()?.worktree(0)
        )),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_worktree_that_is_already_there_and_correct_is_adopted_without_recreating_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let host = worktree_host(CreationMode::Detached, 1)?.holding(&[&layout()?.worktree(0)]);
    let project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let managed = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .required_because("adopt")?;

    assert_eq!(managed.len(), 1);
    assert!(
        !host.ran("worktree add"),
        "an interrupted creation is adopted rather than repeated"
    );
    Ok(())
}

#[test]
fn a_worktree_of_another_repository_is_not_taken_for_this_project() -> Checked {
    // modeの検査はdetached HEADを`symbolic-ref`の失敗で判定するため、共有repository
    // から離れたdirectoryもそれだけでは通ってしまう。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let path = layout()?.worktree(0);
    let host = worktree_host(CreationMode::Detached, 1)?
        .holding(&[&path])
        .answering(
            &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
            "/home/agent/work/elsewhere/.git\n",
        );

    let project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).ok();
    let error = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .refused_because("a worktree of another repository is not this project's")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
    Ok(())
}
