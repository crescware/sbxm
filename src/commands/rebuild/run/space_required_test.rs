use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::Fixture;

#[test]
fn checkout_entries_and_transfer_scratch_are_counted_without_writing_to_the_old_sandbox() -> Checked
{
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    project.metadata.provisioning.requested_worktrees = 3;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = FakeSbx::listing("[]").answering(
        &format!(
            "exec old -- git --git-dir {} worktree list --porcelain -z",
            layout.bare_git_dir()
        ),
        0,
        "worktree /repo\0bare\0\0",
    );
    let (kib, entries) = space_required(&host, "old", &project.metadata).required()?;
    assert_eq!(kib, 2048 + 128 * 1024);
    assert_eq!(entries, 256);
    assert!(!host.ran("update-ref") && !host.ran("mktemp"));
    assert_eq!(
        tree_size("100644 blob abc 4096\ta/b\n160000 commit def -\tsubmodule\n").required()?,
        (4096, 2)
    );
    for malformed in [
        "no path",
        "bad\tpath",
        "100644 blob abc x\tpath",
        "100644 tree abc 4\tpath",
    ] {
        tree_size(malformed).refused_because("an unreadable size must not be counted as zero")?;
    }
    project.metadata.provisioning.start_ref = None;
    space_required(&host, "old", &project.metadata)
        .refused_because("the checkout branch must be known")?;
    Ok(())
}

#[test]
fn the_largest_saved_or_origin_checkout_is_used_and_missing_heads_fail_closed() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    project.metadata.provisioning.requested_worktrees = 3;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let git_dir = layout.bare_git_dir();
    let list = format!("exec old -- git --git-dir {git_dir} worktree list --porcelain -z");
    let tree = format!(
        "exec old -- env GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1 git --git-dir {git_dir} ls-tree -rl"
    );
    let host = FakeSbx::listing("[]")
        .answering(&list, 0, "worktree /repo\0bare\0\0")
        .answering(
            &format!("{tree} refs/heads/main"),
            0,
            "100644 blob abc 8388608\tdir/file\n",
        )
        .answering(
            &format!("{tree} refs/remotes/origin/main"),
            0,
            "100644 blob def 1048576\tfile\n",
        );
    assert_eq!(
        space_required(&host, "old", &project.metadata).required()?,
        (2048 + 3 * 8192 + 128 * 1024, 262)
    );
    let host = host.answering(
        &format!("exec old -- git --git-dir {git_dir} show-ref --verify --quiet refs/heads/main"),
        128,
        "",
    );
    space_required(&host, "old", &project.metadata)
        .refused_because("a failed ref probe cannot mean the ref is absent")?;
    for bad in [
        "worktree /tree\0branch refs/heads/main\0\0",
        "worktree /tree\0HEAD bad\0detached\0\0",
    ] {
        let host = FakeSbx::listing("[]").answering(&list, 0, bad);
        checkout_heads(&host, "old", &git_dir)
            .refused_because("unknown worktree tips must not disappear from the estimate")?;
    }
    Ok(())
}
