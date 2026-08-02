use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::{Fact, SilentProgress};
use crate::diagnostics::{Error, ErrorId};
use crate::metadata;
use crate::metadata::CreationMode;
use crate::msg;
use crate::testing::repository::*;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::{COMMIT, MOVED};

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

#[test]
fn a_worktree_that_was_just_made_is_refused_when_it_is_not_on_the_starting_commit() -> Checked {
    // 作った直後のworktreeは起点commitにいるはずである。そこにいないものは、この
    // 案件がこの実行で作った成果物ではない。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let path = layout()?.worktree(0);
    let host = worktree_host(CreationMode::Detached, 1)?
        .answering(&format!("git -C {path} rev-parse HEAD"), MOVED);

    let project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .refused_because("a worktree that is not on the starting commit is not this run's")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { value, .. }
                if value.id == "cause-head-differs"
                    && value.args.contains(&("observed", MOVED.to_string()))
                    && value.args.contains(&("expected", COMMIT.to_string()))
        )),
        "both the commit that was found and the one that was asked for are named: {:?}",
        diagnostic.facts
    );
    assert!(
        host.ran(&format!("worktree add --detach {path}")),
        "the mismatch is a post-condition of the creation, not a reason to skip it: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn an_attached_worktree_that_sits_on_another_branch_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let path = layout()?.worktree(0);
    let host = worktree_host(CreationMode::Attached, 1)?.answering(
        &format!("git -C {path} symbolic-ref -q HEAD"),
        "refs/heads/other\n",
    );

    let project = metadata(CreationMode::Attached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .refused_because("an attached worktree that took another branch is not the declared one")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { value, .. }
                if value.id == "cause-worktree-not-on-branch"
                    && value
                        .args
                        .contains(&("expected", "refs/heads/develop".to_string()))
        )),
        "the branch that was declared is named: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_detached_worktree_that_ended_up_on_a_branch_is_refused() -> Checked {
    // detachedで作ったworktreeがbranchを持っていれば、それは別のものである。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let path = layout()?.worktree(0);
    let host = worktree_host(CreationMode::Detached, 1)?.answering(
        &format!("git -C {path} symbolic-ref -q HEAD"),
        "refs/heads/develop\n",
    );

    let project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .refused_because("a detached worktree that holds a branch is not the declared one")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { value, .. }
                if value.id == "cause-worktree-on-a-branch"
                    && value
                        .args
                        .contains(&("observed", "refs/heads/develop".to_string()))
        )),
        "the branch the worktree ended up holding is named: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_worktree_left_by_an_interrupted_creation_is_taken_over_rather_than_made_again() -> Checked {
    // 作成の途中で止まった実行が残したworktreeは、この案件の成果物である。作り直すと
    // そこにある作業を消すため、起点commitとmodeだけを確かめて引き継ぐ。
    let path = layout()?.worktree(0);
    let host = worktree_host(CreationMode::Detached, 1)?.holding(&[&path]);

    provision_worktree(
        &host,
        "sbxm-example",
        &layout()?.bare_git_dir(),
        &path,
        "develop",
        CreationMode::Detached,
        COMMIT,
    )
    .required_because("a worktree that is already on the starting commit is usable as it is")?;

    assert!(
        !host.ran("worktree add"),
        "the worktree that is already there is taken over, not remade: {:?}",
        host.calls()
    );
    assert!(
        host.ran(&format!("git -C {path} symbolic-ref -q HEAD")),
        "the mode of the adopted worktree is still checked: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_question_the_host_could_not_ask_is_not_read_as_a_worktree_that_belongs_elsewhere() -> Checked {
    // 起点commitの読み出しも、worktreeの帰属の確認も、答えが返らなければ何も観測
    // していない。それを不一致として扱うと、無事なworktreeを使えないものと告げる。
    let git_dir = layout()?.bare_git_dir();
    let path = layout()?.worktree(0);
    let steps = [
        format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/develop"),
        format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
    ];

    for step in steps {
        let host = worktree_host(CreationMode::Detached, 1)?
            .holding(&[&path])
            .timing_out(&step);
        let project = metadata(CreationMode::Detached, Some("develop"), 1)?;

        let error = ensure_worktrees(
            &host,
            "sbxm-example",
            &layout()?,
            &project,
            "develop",
            &mut SilentProgress,
        )
        .refused_because("a question that went unanswered stops the run")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandTimeout),
            "{step} was reported as something other than the host failure it was"
        );
        assert!(
            !host.ran("worktree add"),
            "nothing is remade while the observation is missing: {:?}",
            host.calls()
        );
    }
    Ok(())
}

/// modeの問い合わせだけがhostへ届かないsandbox。
struct UnobservableMode {
    inner: InnerCommandSandbox,
}

impl HostEnvironment for UnobservableMode {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> crate::diagnostics::Result<CommandOutcome> {
        if crate::testing::command::inner_args(spec).contains(&"symbolic-ref") {
            return Err(Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            ));
        }
        self.inner.run(spec)
    }
}

#[test]
fn a_mode_that_could_not_be_asked_for_is_not_read_as_a_detached_head() -> Checked {
    // detachedの判定は`symbolic-ref`の失敗で行う。問い合わせ自体が届かなかったことを
    // 同じ失敗として扱うと、観測できなかったworktreeが検査を通ってしまう。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let host = UnobservableMode {
        inner: worktree_host(CreationMode::Detached, 1)?,
    };

    let project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = ensure_worktrees(
        &host,
        "sbxm-example",
        &layout()?,
        &project,
        "develop",
        &mut SilentProgress,
    )
    .refused_because("a mode that could not be observed is not a mode that matched")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}
