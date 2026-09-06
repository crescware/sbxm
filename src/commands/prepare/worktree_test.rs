//! managed worktreeの作成と、観測できたHEAD。
use crate::diagnostics::ErrorId;
use crate::metadata::CreationMode;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{
    super::fake::{Bench, World},
    *,
};
use crate::design::SilentProgress;
use crate::testing::add_request::{project_of, request};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::value::COMMIT;

#[test]
fn a_head_that_cannot_be_read_is_refused_rather_than_left_unknown() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first run builds")?;

    // 停止中のSandboxと同じく、worktreeのHEADだけが読めない状態にする。読めない読み取りも
    // 出力は返すので、成功したかどうかはexit statusでしか分からない。observeできない
    // post-conditionを`already_built`へ丸めず、明示的なrepairへ送る。
    world.failing_with("rev-parse HEAD", "fatal: not a git repository\n");
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("an unreadable worktree head is not a verified post-condition")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
    Ok(())
}

#[test]
fn a_head_that_reads_back_empty_is_refused_rather_than_left_unknown() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first run builds")?;

    // 成功しながら何も答えない読み取り。値がない以上、観測できたことにはならない。
    world.succeeding_silently("rev-parse HEAD");
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("an empty answer is not a verified post-condition")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
    Ok(())
}

#[test]
fn three_detached_worktrees_start_from_one_commit_of_the_named_branch() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(3), Some("develop"))?;

    let output = bench.build(&world, &request).required_because("build")?;
    assert_eq!(output.mode, CreationMode::Detached);
    assert_eq!(output.start_ref, "develop");
    assert_eq!(output.worktrees.len(), 3);
    for (index, worktree) in output.worktrees.iter().enumerate() {
        assert_eq!(worktree.path, format!("example-repo.tree-{index}"));
        assert_eq!(worktree.created_from, "refs/remotes/origin/develop");
        assert_eq!(worktree.head, COMMIT);
        assert!(
                world.ran(&format!(
                    "worktree add --detach /home/agent/work/example-repo/example-repo.tree-{index} refs/remotes/origin/develop"
                )),
                "{:?}",
                world.invocations()
            );
    }
    // bare repositoryとworktreeは、1 treeでも3 treesでも分かれている。
    assert!(world.ran("git init --bare /home/agent/work/example-repo/.git"));
    assert!(world.ran("remote add origin https://github.com/Example-Org/Example-Repo.git"));
    Ok(())
}
