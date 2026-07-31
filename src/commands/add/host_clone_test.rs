use super::*;
use crate::command::{CommandOutcome, HostEnvironment};
use crate::paths::AbsoluteBasePath;
use crate::testing::project::{https_repository, ssh_repository};
use crate::ui::SilentProgress;
use std::cell::RefCell;
use std::collections::HashMap;

/// gitの応答を差し替え、起動された内容を記録するhost。
struct FakeGit {
    answers: HashMap<String, String>,
    calls: RefCell<Vec<CommandSpec>>,
    clone_creates: Option<PathBuf>,
}

impl FakeGit {
    fn new() -> FakeGit {
        FakeGit {
            answers: HashMap::new(),
            calls: RefCell::new(Vec::new()),
            clone_creates: None,
        }
    }

    fn answering(mut self, args: &str, stdout: &str) -> FakeGit {
        self.answers.insert(args.to_string(), stdout.to_string());
        self
    }

    /// cloneが成功したときに作られるworking treeを模す。
    fn cloning_into(mut self, path: &Path) -> FakeGit {
        self.clone_creates = Some(path.to_path_buf());
        self
    }

    fn args_of_calls(&self) -> Vec<Vec<String>> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }
}

impl HostEnvironment for FakeGit {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let key = spec.args.join(" ");
        if key.starts_with("clone")
            && let Some(path) = &self.clone_creates
        {
            fs::create_dir_all(path.join(".git")).expect("create the cloned working tree");
        }
        let stdout = self.answers.get(&key).cloned().unwrap_or_default();
        Ok(crate::testing::command::outcome(spec, 0, &stdout))
    }
}

fn project_paths(dir: &Path) -> (ProjectPaths, RepositoryIdentity) {
    let base = AbsoluteBasePath::new(dir).expect("valid base path");
    let repository = ssh_repository("Example-Org/Example-Repo");
    let paths = ProjectPaths::derive(&base, repository.canonical_id());
    fs::create_dir_all(paths.root()).expect("create the project root");
    (paths, repository)
}

/// 対象cloneとして通る応答をまとめる。
fn healthy(clone: &Path) -> FakeGit {
    FakeGit::new()
        .answering("rev-parse --is-bare-repository", "false\n")
        .answering(
            "rev-parse --show-toplevel",
            &format!("{}\n", clone.display()),
        )
        .answering(
            "config --get-all remote.origin.url",
            "git@github.com:Example-Org/Example-Repo.git\n",
        )
}

#[test]
fn a_missing_clone_is_created_from_the_ssh_remote_and_then_verified() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());

    let clone =
        ensure(&host, &paths, &repository, &mut SilentProgress).expect("the clone is created");
    assert!(clone.created);
    assert_eq!(clone.path, paths.host_clone());

    let calls = host.args_of_calls();
    assert_eq!(
        calls[0],
        vec![
            "clone".to_string(),
            "git@github.com:Example-Org/Example-Repo.git".to_string(),
            paths::display(&paths.host_clone()),
        ],
        "the display casing of the project reaches the remote URL"
    );
    assert!(
        calls.len() > 1,
        "the clone is verified after it is created: {calls:?}"
    );
}

#[test]
fn the_clone_forwards_its_progress_while_the_checks_capture_their_output() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());
    ensure(&host, &paths, &repository, &mut SilentProgress).expect("the clone is created");

    let calls = host.calls.borrow();
    let clone = &calls[0];
    assert_eq!(clone.output, crate::command::OutputPolicy::Passthrough);
    assert_eq!(clone.timeout, TimeoutClass::RepositoryTransfer);
    assert_eq!(
        clone.working_dir, None,
        "the clone creates its own directory"
    );
    for inspection in calls.iter().skip(1) {
        assert_eq!(inspection.output, crate::command::OutputPolicy::Capture);
        assert_eq!(
            inspection.working_dir.as_deref(),
            Some(paths.host_clone().as_path()),
            "every check runs inside the clone"
        );
    }
}

#[test]
fn an_existing_clone_of_the_same_repository_is_reused_without_cloning_again() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
    let host = healthy(&paths.host_clone());

    let clone = ensure(&host, &paths, &repository, &mut SilentProgress)
        .expect("the existing clone is reused");
    assert!(!clone.created);
    assert!(
        !host
            .args_of_calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "clone")),
        "an existing clone is never cloned over"
    );
}

#[test]
fn a_dirty_working_tree_does_not_stop_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
    fs::write(paths.host_clone().join("uncommitted.txt"), b"work").unwrap();

    ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .expect("uncommitted work is the user's, not a reason to stop");
}

#[test]
fn a_clone_of_another_repository_is_refused_instead_of_being_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "git@github.com:other-org/other-repo.git\n",
    );

    let error = ensure(&host, &paths, &repository, &mut SilentProgress)
        .expect_err("a different remote is refused");
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert!(
        paths.host_clone().join(".git").exists(),
        "nothing is deleted when the clone cannot be used"
    );
}

#[test]
fn an_ambiguous_or_missing_origin_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

    for urls in [
        "",
        "git@github.com:Example-Org/Example-Repo.git\nhttps://github.com/example-org/example-repo.git\n",
    ] {
        let host =
            healthy(&paths.host_clone()).answering("config --get-all remote.origin.url", urls);
        let error = ensure(&host, &paths, &repository, &mut SilentProgress)
            .expect_err("origin must name exactly one remote");
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    }
}

#[test]
fn a_bare_repository_or_a_nested_working_tree_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

    let bare = healthy(&paths.host_clone()).answering("rev-parse --is-bare-repository", "true\n");
    let error = ensure(&bare, &paths, &repository, &mut SilentProgress)
        .expect_err("a bare repository has no worktree");
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

    // 期待pathの中に居ても、top-levelが別のdirectoryなら再利用しない。
    let nested = healthy(&paths.host_clone()).answering(
        "rev-parse --show-toplevel",
        &format!("{}\n", dir.path().display()),
    );
    let error = ensure(&nested, &paths, &repository, &mut SilentProgress)
        .expect_err("the clone must be its own working tree");
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
}

#[test]
fn a_git_directory_that_points_outside_the_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone()).unwrap();
    let elsewhere = dir.path().join("elsewhere.git");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(
        paths.host_clone().join(".git"),
        format!("gitdir: {}\n", elsewhere.display()),
    )
    .unwrap();

    let error = ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .expect_err("a worktree file that leaves the project is refused");
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

    // 案件directoryの中を指すworktree fileは受け入れる。
    let inside = paths.root().join(".sbxm").join("worktree.git");
    fs::create_dir_all(&inside).unwrap();
    fs::write(
        paths.host_clone().join(".git"),
        format!("gitdir: {}\n", inside.display()),
    )
    .unwrap();
    ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .expect("a git directory inside the project is part of the project");
}

#[test]
fn an_https_registration_clones_over_https() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).expect("valid base path");
    let repository = https_repository("Example-Org/Example-Repo");
    let paths = ProjectPaths::derive(&base, repository.canonical_id());
    fs::create_dir_all(paths.root()).expect("create the project root");

    let host = healthy(&paths.host_clone())
        .answering(
            "config --get-all remote.origin.url",
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .cloning_into(&paths.host_clone());

    ensure(&host, &paths, &repository, &mut SilentProgress).expect("the clone is created");
    assert_eq!(
        host.args_of_calls()[0],
        vec![
            "clone".to_string(),
            "https://github.com/Example-Org/Example-Repo.git".to_string(),
            paths::display(&paths.host_clone()),
        ],
        "the declared transport is the one the clone uses"
    );
}

#[test]
fn an_origin_that_names_the_same_repository_over_another_transport_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "https://github.com/Example-Org/Example-Repo.git\n",
    );
    let error = ensure(&host, &paths, &repository, &mut SilentProgress)
        .expect_err("SSH and HTTPS are not the same configuration");
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
}

#[test]
fn an_origin_only_the_display_casing_differs_in_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "git@github.com:example-org/example-repo.git\n",
    );
    ensure(&host, &paths, &repository, &mut SilentProgress)
        .expect("only the display casing differs, so it is the same repository");
}

#[test]
fn an_origin_that_is_not_one_of_the_accepted_forms_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (paths, repository) = project_paths(dir.path());
    fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

    for url in [
        "git@github.com:Example-Org/Example-Repo\n",
        "ssh://git@github.com/Example-Org/Example-Repo.git\n",
        "/srv/git/example-repo.git\n",
    ] {
        let host =
            healthy(&paths.host_clone()).answering("config --get-all remote.origin.url", url);
        let error = ensure(&host, &paths, &repository, &mut SilentProgress)
            .expect_err("an origin sbxm cannot read is never assumed to match");
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    }
}
