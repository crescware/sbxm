//! 中断した構築を、同じ`prepare`が続きから進める。
use crate::metadata::CreationMode;

use crate::testing::outcome::{Checked, Refused, Required};

use super::super::fake::{Bench, World};
use crate::compatibility::SandboxState;
use crate::diagnostics::ErrorId;
use crate::support::files::Placement;
use crate::testing::add_request::request;
use crate::testing::value::COMMIT;

/// `add`と`prepare`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
const STEPS: [(&str, ErrorId); 11] = [
    ("git clone git@github.com", ErrorId::ExternalCommandFailed),
    ("docker build", ErrorId::ExternalCommandFailed),
    ("docker image save", ErrorId::ExternalCommandFailed),
    ("sbx template load", ErrorId::ExternalCommandFailed),
    ("sbx create", ErrorId::ExternalCommandFailed),
    ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
    ("config --global user.name", ErrorId::ExternalCommandFailed),
    ("sbx secret ls", ErrorId::ExternalCommandFailed),
    ("git init --bare", ErrorId::ExternalCommandFailed),
    ("check-ref-format", ErrorId::InvalidBranchName),
    ("worktree add", ErrorId::ExternalCommandFailed),
];

#[test]
fn an_interruption_at_any_step_is_continued_by_the_same_prepare() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    // 1工程ずつ後ろへずらして失敗させる。次の実行がそこまで進めることが継続の証拠になる。
    for (step, expected) in STEPS {
        world.failing(step);
        let error = bench
            .build(&world, &request)
            .refused_because("the run stops at the step that failed")?;
        assert_eq!(error.first_id(), Some(expected), "{step}");
        world.nothing_fails();
    }

    // 最後に失敗したのはworktree作成であり、続きの実行はそこから進む。
    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .required_because("the same add finishes")?;
    let tail = world.since(mark);

    assert!(!output.already_built);
    assert_eq!(output.mode, CreationMode::Attached);
    assert_eq!(output.start_ref, "main");
    assert_eq!(output.sandbox_state, SandboxState::Running);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(output.worktrees[0].path, "example-repo.tree-0");
    assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT));
    assert_eq!(output.files.len(), 1);
    assert_eq!(
        output.files[0].placement,
        Placement::Unchanged,
        "an earlier run placed the file, and an identical destination is left alone"
    );

    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert_eq!(stored.provisioning.start_ref.as_deref(), Some("main"));

    // 成功済みの成果物は作り直さない。
    for done in [
        "git clone git@github.com",
        "docker build",
        "sbx template load",
        "sbx create",
        "git init --bare",
    ] {
        assert!(
            !tail.iter().any(|call| call.contains(done)),
            "{done} was already done: {tail:?}"
        );
    }
    assert_eq!(
        tail.iter()
            .filter(|call| call.contains("worktree add"))
            .count(),
        1,
        "the run continues with the step that had failed: {tail:?}"
    );
    // archiveは工程へ到達するたびに作り直す。
    assert!(tail.iter().any(|call| call.contains("docker image save")));
    Ok(())
}

#[test]
fn a_finished_build_is_a_no_op_for_the_same_add() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first run builds")?;

    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .required_because("the second run changes nothing")?;

    assert!(output.already_built);
    for forbidden in [
        "docker build",
        "docker image save",
        "sbx template load",
        "sbx create",
        "sbx daemon stop",
        "sbx cp",
        "worktree add",
        "git clone",
    ] {
        assert!(
            !world
                .since(mark)
                .iter()
                .any(|call| call.contains(forbidden)),
            "a finished project must not run {forbidden}: {:?}",
            world.since(mark)
        );
    }
    Ok(())
}
