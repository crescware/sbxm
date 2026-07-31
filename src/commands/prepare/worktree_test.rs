//! managed worktreeの作成と、観測できたHEAD。
use super::super::world::{World, bench};
use super::*;
use crate::testing::add_request::{project_of, request};
use crate::testing::value::COMMIT;
use crate::ui::SilentProgress;

#[test]
fn a_head_that_cannot_be_read_is_left_unknown() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    // 停止中のSandboxと同じく、worktreeのHEADだけが読めない状態にする。読めない読み取りも
    // 出力は返すので、成功したかどうかはexit statusでしか分からない。
    world.failing_with("rev-parse HEAD", "fatal: not a git repository\n");
    let output = run(
        &bench.location,
        &bench.config,
        &project_of(&request),
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .expect("a project that is built stays built");

    assert!(output.already_built);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        output.worktrees[0].head, None,
        "the output of a failed read is not reported as a HEAD"
    );
    assert_eq!(
        output.worktrees[0].created_from, "refs/remotes/origin/main",
        "what metadata declares is still reported"
    );
    assert_eq!(output.worktrees[0].mode, CreationMode::Attached);
}

#[test]
fn a_head_that_reads_back_empty_is_left_unknown() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    // 成功しながら何も答えない読み取り。値がない以上、観測できたことにはならない。
    world.succeeding_silently("rev-parse HEAD");
    let output = run(
        &bench.location,
        &bench.config,
        &project_of(&request),
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .expect("a project that is built stays built");

    assert!(output.already_built);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        output.worktrees[0].head, None,
        "an empty answer is not reported as a HEAD"
    );
}

#[test]
fn three_detached_worktrees_start_from_one_commit_of_the_named_branch() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(3), Some("develop"));

    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.mode, CreationMode::Detached);
    assert_eq!(output.start_ref, "develop");
    assert_eq!(output.worktrees.len(), 3);
    for (index, worktree) in output.worktrees.iter().enumerate() {
        assert_eq!(worktree.path, format!("example-repo.tree-{index}"));
        assert_eq!(worktree.created_from, "refs/remotes/origin/develop");
        assert_eq!(worktree.head.as_deref(), Some(COMMIT));
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
}
