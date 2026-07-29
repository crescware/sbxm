use super::*;
use crate::metadata::Provisioning;
use crate::paths::AbsoluteBasePath;
use crate::project::CanonicalProjectId;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

fn project() -> ProjectId {
    ProjectId::parse("Example-Org/Example-Repo").expect("valid project id")
}

fn canonical() -> CanonicalProjectId {
    project().canonical()
}

fn layout() -> SandboxLayout {
    SandboxLayout::new(&canonical())
}

/// bare cloneの検査を通る応答。
fn healthy_clone() -> InnerCommandSandbox {
    let git_dir = layout().bare_git_dir();
    InnerCommandSandbox::new()
        .answering(
            &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
            "true\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            &format!("{FETCH_REFSPEC}\n"),
        )
}

fn metadata(mode: CreationMode, start_ref: Option<&str>, count: u32) -> ProjectMetadata {
    ProjectMetadata {
        owner: "Example-Org".to_string(),
        repository: "Example-Repo".to_string(),
        canonical_id: canonical(),
        provisioning: Provisioning {
            mode,
            start_ref: start_ref.map(|value| value.to_string()),
            requested_worktrees: count,
            dockerfile_sha256: "1".repeat(64),
        },
        rebuild: None,
    }
}

fn project_paths(dir: &std::path::Path) -> ProjectPaths {
    let base = AbsoluteBasePath::new(dir).expect("valid base path");
    let paths = ProjectPaths::derive(&base, &canonical());
    std::fs::create_dir_all(paths.sbxm_dir()).expect("create .sbxm");
    paths
}

#[test]
fn a_missing_repository_is_cloned_bare_over_https_and_then_verified() {
    let host = healthy_clone();
    ensure_bare_clone(&host, "sbxm-example", &project(), &layout()).expect("clone");

    assert!(
        host.ran("git init --bare /home/agent/work/example-repo/.git"),
        "{:?}",
        host.calls()
    );
    assert!(host.ran("remote add origin https://github.com/Example-Org/Example-Repo.git"));
    assert!(host.ran(&format!("config remote.origin.fetch {FETCH_REFSPEC}")));
    assert!(host.ran("fetch --prune origin"));
    assert!(
        host.ran("mkdir -p /home/agent/work/example-repo"),
        "the bare repository lives below the work directory"
    );
}

#[test]
fn an_existing_repository_of_the_same_project_is_reused() {
    let git_dir = layout().bare_git_dir();
    let host = healthy_clone().holding(&[&git_dir]);
    ensure_bare_clone(&host, "sbxm-example", &project(), &layout()).expect("reuse");

    assert!(
        !host.ran("git clone"),
        "an existing repository is not recloned"
    );
    assert!(host.ran("fetch --prune origin"));
}

#[test]
fn a_repository_that_does_not_match_is_refused_instead_of_being_replaced() {
    let git_dir = layout().bare_git_dir();

    let cases = [
        healthy_clone().answering(
            &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
            "false\n",
        ),
        healthy_clone().answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            "https://github.com/other-org/other-repo.git\n",
        ),
        healthy_clone().answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            "+refs/heads/main:refs/remotes/origin/main\n",
        ),
        healthy_clone().failing(&format!("git --git-dir {git_dir} fsck --connectivity-only")),
    ];

    for host in cases {
        let host = host.holding(&[&git_dir]);
        let error = ensure_bare_clone(&host, "sbxm-example", &project(), &layout())
            .expect_err("a repository that cannot be proven is refused");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
        assert!(!host.ran("rm "), "nothing is deleted: {:?}", host.calls());
    }
}

#[test]
fn an_attached_project_resolves_the_remote_default_branch_and_records_it() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let git_dir = layout().bare_git_dir();
    let host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
        "ref: refs/heads/main\tHEAD\n9f5b1c\tHEAD\n",
    );

    let mut project = metadata(CreationMode::Attached, None, 1);
    metadata::create(&paths, &project).expect("write the metadata");

    let branch =
        resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project).expect("resolve");
    assert_eq!(branch, "main");
    assert_eq!(project.provisioning.start_ref.as_deref(), Some("main"));

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(
        stored.provisioning.start_ref.as_deref(),
        Some("main"),
        "the resolved branch is written before any worktree is made"
    );
    assert!(host.ran("git check-ref-format --branch main"));
    assert!(host.ran("show-ref --verify --quiet refs/remotes/origin/main"));
}

#[test]
fn a_head_that_points_at_something_other_than_a_branch_is_not_a_start_point() {
    let git_dir = layout().bare_git_dir();
    let cases = [
        // branch以外を指すHEAD。
        "ref: refs/tags/v1.0.0\tHEAD\n9f5b1c\tHEAD\n",
        // branchのpathだが、名前として渡せない綴り。
        "ref: refs/heads/-oops\tHEAD\n9f5b1c\tHEAD\n",
    ];

    for answer in cases {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths(dir.path());
        let host = InnerCommandSandbox::new().answering(
            &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
            answer,
        );

        let mut project = metadata(CreationMode::Attached, None, 1);
        metadata::create(&paths, &project).expect("write the metadata");

        let error = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
            .expect_err("only a branch can be the start point");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

        let stored = metadata::load(&paths).unwrap().expect("present");
        assert_eq!(
            stored.provisioning.start_ref, None,
            "nothing is adopted from a HEAD that names no branch"
        );
        assert!(
            !host.ran("check-ref-format"),
            "a name that is not a branch is never handed to git: {:?}",
            host.calls()
        );
    }
}

#[test]
fn the_start_branch_is_judged_again_by_git_inside_the_sandbox() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    // hostのvalidationは通るが、gitがbranch名として受け付けない値。
    let host = InnerCommandSandbox::new().failing("git check-ref-format --branch feature..login");

    let mut project = metadata(CreationMode::Detached, Some("feature..login"), 1);
    metadata::create(&paths, &project).expect("write the metadata");

    let error = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
        .expect_err("git has the final say on what is a branch name");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));
    assert!(
        !host.ran("show-ref"),
        "a name git refuses is never looked up: {:?}",
        host.calls()
    );
}

#[test]
fn a_resolved_branch_that_git_refuses_is_not_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let git_dir = layout().bare_git_dir();
    let host = InnerCommandSandbox::new()
        .answering(
            &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
            "ref: refs/heads/main\tHEAD\n",
        )
        .failing("git check-ref-format --branch main");

    let mut project = metadata(CreationMode::Attached, None, 1);
    metadata::create(&paths, &project).expect("write the metadata");

    let error = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
        .expect_err("an unusable name is not adopted");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(
        stored.provisioning.start_ref, None,
        "the target configuration keeps waiting for a branch it can use"
    );
}

#[test]
fn a_start_branch_that_has_no_remote_tracking_ref_stops_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let git_dir = layout().bare_git_dir();
    let host = InnerCommandSandbox::new().failing(&format!(
        "git --git-dir {git_dir} show-ref --verify --quiet refs/remotes/origin/develop"
    ));

    let mut project = metadata(CreationMode::Detached, Some("develop"), 1);
    metadata::create(&paths, &project).expect("write the metadata");

    let error = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
        .expect_err("a branch that is not on the remote cannot be a start point");
    assert_eq!(error.first_id(), Some(ErrorId::StartRefUnresolved));
}

/// worktreeの検査を通る応答。
fn worktree_host(mode: CreationMode, count: u32) -> InnerCommandSandbox {
    let git_dir = layout().bare_git_dir();
    let mut host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/develop"),
        &format!("{COMMIT}\n"),
    );
    for index in 0..count {
        let path = layout().worktree(index);
        host = host.answering(
            &format!("git -C {path} rev-parse HEAD"),
            &format!("{COMMIT}\n"),
        );
        host = host.answering(
            &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
            &format!("{}\n", layout().bare_git_dir()),
        );
        host = match mode {
            CreationMode::Attached => host.answering(
                &format!("git -C {path} symbolic-ref -q HEAD"),
                "refs/heads/develop\n",
            ),
            CreationMode::Detached => host.failing(&format!("git -C {path} symbolic-ref -q HEAD")),
        };
    }
    host
}

/// 記録済みworktreeが起点から離れている状態。作業すればこうなる。
const MOVED: &str = "1a2b3c4d5e6f708192a3b4c5d6e7f80912a3b4c5";

#[test]
fn a_project_that_asks_for_more_worktrees_gets_the_missing_ones_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let existing = layout().worktree(0);
    // 既にあるtree-0はcommitを重ねて起点から離れている。増設のためにこれを作り直す
    // ことも、離れていることを理由に止まることもあってはならない。
    let host = worktree_host(CreationMode::Detached, 3)
        .holding(&[&existing])
        .answering(&format!("git -C {existing} rev-parse HEAD"), MOVED);

    let project = metadata(CreationMode::Detached, Some("develop"), 3);
    metadata::create(&paths, &project).expect("write the metadata");

    let managed = ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop")
        .expect("the worktrees that are missing are the ones that get made");

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
                layout().worktree(index)
            )),
            "{:?}",
            host.calls()
        );
    }
}

#[test]
fn an_attached_project_keeps_its_branch_and_gets_detached_worktrees_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let existing = layout().worktree(0);
    // tree-0はbranchを持ったまま。Gitは同じbranchを2つのworktreeへcheckoutさせない
    // ため、足す側はdetachedになる。案件全体を移す必要はない。
    let host = worktree_host(CreationMode::Detached, 3)
        .holding(&[&existing])
        .answering(&format!("git -C {existing} rev-parse HEAD"), MOVED)
        .answering(
            &format!("git -C {existing} symbolic-ref -q HEAD"),
            "refs/heads/develop\n",
        );

    let project = metadata(CreationMode::Attached, Some("develop"), 3);
    metadata::create(&paths, &project).expect("write the metadata");

    let managed = ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop")
        .expect("an attached worktree does not stop the others from being made");

    assert_eq!(managed.len(), 3);
    for index in 1..3 {
        assert!(
            host.ran(&format!(
                "worktree add --detach {} refs/remotes/origin/develop",
                layout().worktree(index)
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
}

#[test]
fn detached_worktrees_are_created_from_one_commit_and_recorded_one_by_one() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let host = worktree_host(CreationMode::Detached, 3);
    let project = metadata(CreationMode::Detached, Some("develop"), 3);
    metadata::create(&paths, &project).expect("write the metadata");

    let managed =
        ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop").expect("create");

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
                layout().worktree(index)
            )),
            "{:?}",
            host.calls()
        );
    }
}

#[test]
fn an_attached_project_gets_one_tracking_branch() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let host = worktree_host(CreationMode::Attached, 1);
    let project = metadata(CreationMode::Attached, Some("develop"), 1);
    metadata::create(&paths, &project).expect("write the metadata");

    ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop").expect("create");

    assert!(
        host.ran(&format!(
            "worktree add --track -b develop {} refs/remotes/origin/develop",
            layout().worktree(0)
        )),
        "{:?}",
        host.calls()
    );
}

#[test]
fn a_worktree_that_is_already_there_and_correct_is_adopted_without_recreating_it() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let host = worktree_host(CreationMode::Detached, 1).holding(&[&layout().worktree(0)]);
    let project = metadata(CreationMode::Detached, Some("develop"), 1);
    metadata::create(&paths, &project).expect("write the metadata");

    let managed =
        ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop").expect("adopt");

    assert_eq!(managed.len(), 1);
    assert!(
        !host.ran("worktree add"),
        "an interrupted creation is adopted rather than repeated"
    );
}

#[test]
fn a_worktree_of_another_repository_is_not_taken_for_this_project() {
    // modeの検査はdetached HEADを`symbolic-ref`の失敗で判定するため、共有repository
    // から離れたdirectoryもそれだけでは通ってしまう。
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let path = layout().worktree(0);
    let host = worktree_host(CreationMode::Detached, 1)
        .holding(&[&path])
        .answering(
            &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
            "/home/agent/work/elsewhere/.git\n",
        );

    let project = metadata(CreationMode::Detached, Some("develop"), 1);
    metadata::create(&paths, &project).ok();
    let error = ensure_worktrees(&host, "sbxm-example", &layout(), &project, "develop")
        .expect_err("a worktree of another repository is not this project's");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
}
