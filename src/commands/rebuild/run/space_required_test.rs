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
    let (kib, entries) = space_required(&host, "old", &project.metadata, 4096).required()?;
    assert_eq!(kib, 2048 + 128 * 1024);
    assert_eq!(entries, 256);
    assert!(!host.ran("update-ref") && !host.ran("mktemp"));
    assert_eq!(
        tree_size(
            "100644 blob abc 4096\ta/b\n160000 commit def -\tsubmodule\n",
            4096
        )
        .required()?,
        (12288, 2)
    );
    for malformed in [
        "no path",
        "bad\tpath",
        "100644 blob abc x\tpath",
        "100644 tree abc 4\tpath",
    ] {
        tree_size(malformed, 4096)
            .refused_because("an unreadable size must not be counted as zero")?;
    }
    project.metadata.provisioning.start_ref = None;
    space_required(&host, "old", &project.metadata, 4096)
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
            &format!("exec old -- env GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1 git --git-dir {git_dir} rev-parse --revs-only --end-of-options refs/heads/main^{{tree}} refs/remotes/origin/main^{{tree}}"),
            0,
            &format!("{}\n{}\n", crate::testing::value::COMMIT, "b".repeat(40)),
        )
        .answering(
            &format!("{tree} {}", crate::testing::value::COMMIT),
            0,
            "100644 blob abc 8388608\tdir/file\n",
        )
        .answering(
            &format!("{tree} {}", "b".repeat(40)),
            0,
            "100644 blob def 1048576\tfile\n",
        );
    assert_eq!(
        space_required(&host, "old", &project.metadata, 4096).required()?,
        (2048 + 3 * 8200 + 128 * 1024, 262)
    );
    let host = host.answering(
        &format!("exec old -- git --git-dir {git_dir} show-ref --verify --quiet refs/heads/main"),
        128,
        "",
    );
    space_required(&host, "old", &project.metadata, 4096)
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

#[test]
fn small_files_use_the_target_filesystems_allocation_unit_and_include_metadata() -> Checked {
    let tree = crate::testing::repository::small_file_tree(100_000)?;
    let (bytes, entries) = tree_size(&tree, 4096).required()?;
    assert_eq!(entries, 100_000);
    assert!(
        bytes > 4096 * 100_000,
        "checkout data and metadata need blocks per file"
    );
    assert_eq!(bytes % 4096, 0);
    assert!(tree_size("100644 blob abc 1\tsmall\n", 65536).required()?.0 >= 65536);
    tree_size("", 0).refused_because("an unknown allocation unit must not pass")?;
    Ok(())
}

#[test]
fn aliases_and_distinct_commits_with_the_same_tree_share_one_full_scan() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let git_dir = SandboxLayout::new(project.metadata.canonical_id()).bare_git_dir();
    let host = FakeSbx::listing("[]").answering(
        &format!("exec old -- git --git-dir {git_dir} worktree list --porcelain -z"), 0,
        &format!("worktree /repo\0bare\0\0worktree /one\0HEAD {}\0detached\0\0worktree /two\0HEAD {}\0detached\0\0", crate::testing::value::COMMIT, "b".repeat(40)),
    );
    space_required(&host, "old", &project.metadata, 4096).required()?;
    let calls = host.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|args| args.iter().any(|arg| arg == "ls-tree"))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|args| args.iter().any(|arg| arg == "--end-of-options"))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn missing_malformed_or_failed_tree_resolution_never_starts_a_checkout_scan() -> Checked {
    let sources = std::collections::BTreeSet::from(["main".to_string()]);
    let command = "exec old -- env GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1 git --git-dir /repo rev-parse --revs-only --end-of-options main^{tree}";
    for (status, output) in [(0, ""), (0, "bad\n"), (1, "failure\n")] {
        let host = FakeSbx::listing("[]").answering(command, status, output);
        checkout_trees(&host, "old", "/repo", &sources)
            .refused_because("tree identities are not known")?;
        assert!(!host.ran("ls-tree"));
    }
    Ok(())
}

#[test]
fn real_git_resolves_aliases_and_empty_commits_to_one_tree_and_refuses_missing_refs() -> Checked {
    use crate::testing::repository::git_in;
    use crate::testing::sandbox::LocalSandbox;

    let directory = tempfile::tempdir().required()?;
    git_in(directory.path(), &["init"])?;
    git_in(
        directory.path(),
        &["commit", "--allow-empty", "-m", "first"],
    )?;
    let first = git_in(directory.path(), &["rev-parse", "HEAD"])?;
    git_in(
        directory.path(),
        &["commit", "--allow-empty", "-m", "second"],
    )?;
    let tree = git_in(directory.path(), &["rev-parse", "HEAD^{tree}"])?;
    let git_dir = directory.path().join(".git");
    let sources = std::collections::BTreeSet::from([first, "main".into(), "HEAD".into()]);
    assert_eq!(
        checkout_trees(
            &LocalSandbox,
            "sandbox",
            git_dir.to_str().required()?,
            &sources
        )
        .required()?,
        std::collections::BTreeSet::from([tree])
    );
    checkout_trees(
        &LocalSandbox,
        "sandbox",
        git_dir.to_str().required()?,
        &std::collections::BTreeSet::from(["main".into(), "missing".into()]),
    )
    .refused_because("a missing ref must not disappear from the estimate")?;
    Ok(())
}
