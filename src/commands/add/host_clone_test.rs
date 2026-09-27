use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::CommandOutcome;
use crate::design::SilentProgress;
use crate::paths::ProjectParent;
use crate::testing::host::AnsweredHost;
use crate::testing::project::{https_repository, ssh_repository};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::os::unix::fs::PermissionsExt;

/// gitの応答を差し替え、起動された内容を記録するhost。
struct FakeGit {
    answers: HashMap<String, String>,
    /// gitが128で終わる指定。
    failures: HashSet<String>,
    /// 起動できない指定。先頭の語で一致させる。
    unrunnable: Option<String>,
    calls: RefCell<Vec<CommandSpec>>,
    clone_creates: Option<PathBuf>,
}

impl FakeGit {
    fn new() -> FakeGit {
        FakeGit {
            answers: HashMap::new(),
            failures: HashSet::new(),
            unrunnable: None,
            calls: RefCell::new(Vec::new()),
            clone_creates: None,
        }
    }

    fn answering(mut self, args: &str, stdout: &str) -> FakeGit {
        self.answers.insert(args.to_string(), stdout.to_string());
        self
    }

    /// gitが答えられずに終わる指定。
    fn failing(mut self, args: &str) -> FakeGit {
        self.failures.insert(args.to_string());
        self
    }

    /// 起動できない指定。何も作らず、何も答えない。
    fn unrunnable(mut self, args: &str) -> FakeGit {
        self.unrunnable = Some(args.to_string());
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

impl AnsweredHost for FakeGit {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let key = spec.args.join(" ");
        if self
            .unrunnable
            .as_ref()
            .is_some_and(|args| key.starts_with(args.as_str()))
        {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::ExternalCommandSpawnFailed,
                    msg!("error-external-command-spawn-failed"),
                )
                .fact(Fact::command(&spec.program)),
            ));
        }
        if self.failures.contains(&key) {
            return Ok(crate::testing::command::outcome(spec, 128, ""));
        }
        if key.starts_with("clone")
            && let Some(path) = &self.clone_creates
        {
            // 実物のcloneと同じく、作れなければcloneの失敗として答える。
            if fs::create_dir_all(path.join(".git")).is_err() {
                return Ok(crate::testing::command::outcome(spec, 1, ""));
            }
        }
        let stdout = self.answers.get(&key).cloned().unwrap_or_default();
        Ok(crate::testing::command::outcome(spec, 0, &stdout))
    }
}

/// 拒否が示した`Cause:`のうち、sbxm自身が観測したことを述べるmessage ID。
fn reason_of(error: &Error) -> Checked<&'static str> {
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the refusal names what it observed")
}

/// 拒否が示した`Path:`。読み手が見に行くべきpathである。
fn shown_path(error: &Error) -> Checked<String> {
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-path-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the refusal names the path it looked at")
}

/// 対処が指すpath。
fn remediation_path(error: &Error) -> Checked<String> {
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("the refusal says what to do")?;
    let explanation = remediation
        .explanation
        .first()
        .required_because("the remediation is explained")?;
    explanation
        .args
        .iter()
        .find(|(key, _)| *key == "path")
        .map(|(_, value)| value.clone())
        .required_because("the remediation names a path")
}

fn project_paths(dir: &Path) -> Checked<(ProjectPaths, RepositoryIdentity)> {
    let base = ProjectParent::at(dir).required_because("valid parent directory")?;
    let repository = ssh_repository("Example-Org/Example-Repo")?;
    let paths = ProjectPaths::derive(&base, repository.canonical_id());
    fs::create_dir_all(paths.root()).required_because("create the project root")?;
    Ok((paths, repository))
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
fn a_missing_clone_is_created_from_the_ssh_remote_and_then_verified() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());

    let clone = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .required_because("the clone is created")?;
    assert!(clone.created);
    assert_eq!(clone.path, paths.host_clone());

    let calls = host.args_of_calls();
    assert_eq!(
        calls[0],
        vec![
            "clone".to_string(),
            "--progress".to_string(),
            "git@github.com:Example-Org/Example-Repo.git".to_string(),
            paths::display(&paths.host_clone()),
        ],
        "the display casing of the project reaches the remote URL"
    );
    assert!(
        calls.len() > 1,
        "the clone is verified after it is created: {calls:?}"
    );
    Ok(())
}

#[test]
fn the_clone_forwards_its_progress_while_the_checks_capture_their_output() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());
    HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .required_because("the clone is created")?;

    let calls = host.calls.borrow();
    let clone = &calls[0];
    assert_eq!(clone.output(), crate::boundary::host::OutputPolicy::Relay);
    assert_eq!(clone.timeout, TimeoutClass::RepositoryTransfer);
    assert_eq!(
        clone.working_dir, None,
        "the clone creates its own directory"
    );
    for inspection in calls.iter().skip(1) {
        assert_eq!(
            inspection.output(),
            crate::boundary::host::OutputPolicy::Capture
        );
        assert_eq!(
            inspection.working_dir.as_deref(),
            Some(paths.host_clone().as_path()),
            "every check runs inside the clone"
        );
    }
    Ok(())
}

#[test]
fn an_existing_clone_of_the_same_repository_is_reused_without_cloning_again() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;
    let host = healthy(&paths.host_clone());

    let clone = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .required_because("the existing clone is reused")?;
    assert!(!clone.created);
    assert!(
        !host
            .args_of_calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "clone")),
        "an existing clone is never cloned over"
    );
    Ok(())
}

#[test]
fn a_dirty_working_tree_does_not_stop_the_build() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;
    fs::write(paths.host_clone().join("uncommitted.txt"), b"work").required()?;

    HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .required_because("uncommitted work is the user's, not a reason to stop")?;
    Ok(())
}

#[test]
fn a_clone_of_another_repository_is_refused_instead_of_being_replaced() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;
    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "git@github.com:other-org/other-repo.git\n",
    );

    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a different remote is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert!(
        paths.host_clone().join(".git").exists(),
        "nothing is deleted when the clone cannot be used"
    );
    Ok(())
}

#[test]
fn an_ambiguous_or_missing_origin_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    for urls in [
        "",
        "git@github.com:Example-Org/Example-Repo.git\nhttps://github.com/example-org/example-repo.git\n",
    ] {
        let host =
            healthy(&paths.host_clone()).answering("config --get-all remote.origin.url", urls);
        let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
            .refused_because("origin must name exactly one remote")?;
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    }
    Ok(())
}

#[test]
fn a_bare_repository_or_a_nested_working_tree_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    let bare = healthy(&paths.host_clone()).answering("rev-parse --is-bare-repository", "true\n");
    let error = HostClone::ensure(&bare, &paths, &repository, &mut SilentProgress)
        .refused_because("a bare repository has no worktree")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

    // 期待pathの中に居ても、top-levelが別のdirectoryなら再利用しない。
    let nested = healthy(&paths.host_clone()).answering(
        "rev-parse --show-toplevel",
        &format!("{}\n", dir.path().display()),
    );
    let error = HostClone::ensure(&nested, &paths, &repository, &mut SilentProgress)
        .refused_because("the clone must be its own working tree")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    Ok(())
}

#[test]
fn a_git_directory_that_points_outside_the_project_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;
    let elsewhere = dir.path().join("elsewhere.git");
    fs::create_dir_all(&elsewhere).required()?;
    fs::write(
        paths.host_clone().join(".git"),
        format!("gitdir: {}\n", elsewhere.display()),
    )
    .required()?;

    let error = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .refused_because("a worktree file that leaves the project is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

    // 案件directoryの中を指すworktree fileは受け入れる。
    let inside = paths.root().join(".sbxm").join("worktree.git");
    fs::create_dir_all(&inside).required()?;
    fs::write(
        paths.host_clone().join(".git"),
        format!("gitdir: {}\n", inside.display()),
    )
    .required()?;
    HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .required_because("a git directory inside the project is part of the project")?;
    Ok(())
}

#[test]
fn an_https_registration_clones_over_https() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required_because("valid parent directory")?;
    let repository = https_repository("Example-Org/Example-Repo")?;
    let paths = ProjectPaths::derive(&base, repository.canonical_id());
    fs::create_dir_all(paths.root()).required_because("create the project root")?;

    let host = healthy(&paths.host_clone())
        .answering(
            "config --get-all remote.origin.url",
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .cloning_into(&paths.host_clone());

    HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .required_because("the clone is created")?;
    assert_eq!(
        host.args_of_calls()[0],
        vec![
            "clone".to_string(),
            "--progress".to_string(),
            "https://github.com/Example-Org/Example-Repo.git".to_string(),
            paths::display(&paths.host_clone()),
        ],
        "the declared transport is the one the clone uses"
    );
    Ok(())
}

#[test]
fn an_origin_that_names_the_same_repository_over_another_transport_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "https://github.com/Example-Org/Example-Repo.git\n",
    );
    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("SSH and HTTPS are not the same configuration")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    Ok(())
}

#[test]
fn an_origin_only_the_display_casing_differs_in_is_accepted() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    let host = healthy(&paths.host_clone()).answering(
        "config --get-all remote.origin.url",
        "git@github.com:example-org/example-repo.git\n",
    );
    HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .required_because("only the display casing differs, so it is the same repository")?;
    Ok(())
}

#[test]
fn an_origin_that_is_not_one_of_the_accepted_forms_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    for url in [
        "git@github.com:Example-Org/Example-Repo\n",
        "ssh://git@github.com/Example-Org/Example-Repo.git\n",
        "/srv/git/example-repo.git\n",
    ] {
        let host =
            healthy(&paths.host_clone()).answering("config --get-all remote.origin.url", url);
        let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
            .refused_because("an origin sbxm cannot read is never assumed to match")?;
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    }
    Ok(())
}

#[test]
fn a_symlink_where_the_clone_belongs_is_refused_rather_than_followed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir_all(elsewhere.join(".git")).required()?;
    std::os::unix::fs::symlink(&elsewhere, paths.host_clone()).required()?;

    let host = healthy(&paths.host_clone());
    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a symlinked clone path is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        host.args_of_calls().is_empty(),
        "git never runs through a link that leaves the project: {:?}",
        host.args_of_calls()
    );
    assert!(
        paths.host_clone().is_symlink(),
        "a refusal removes nothing, not even the link"
    );
    Ok(())
}

#[test]
fn a_file_where_the_clone_belongs_is_refused_instead_of_being_cloned_over() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::write(paths.host_clone(), b"not a clone\n").required()?;

    let host = healthy(&paths.host_clone());
    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a regular file is not a working tree")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert_eq!(reason_of(&error)?, "cause-not-a-directory");
    assert_eq!(
        fs::read_to_string(paths.host_clone()).required()?,
        "not a clone\n",
        "what sbxm did not create, it does not delete"
    );
    assert!(host.args_of_calls().is_empty());
    Ok(())
}

#[test]
fn a_directory_holding_no_git_directory_names_the_git_path_it_could_not_read() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;

    let error = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .refused_because("a directory without a git directory is not a clone")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

    // `.git`側の不備は`.git`を示し、対処はcloneのrootを示す。
    assert_eq!(
        shown_path(&error)?,
        paths::display(&paths.host_clone().join(".git"))
    );
    assert_eq!(
        remediation_path(&error)?,
        paths::display(&paths.host_clone())
    );
    // 読めなかった理由はOSが書いた原文であり、言い換えない。
    let diagnostic = &error.diagnostics()[0];
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::OneLine { label, value }
                if label.id == "diagnostic-cause-label" && value.as_str().contains("os error")
        )),
        "the operating system's own wording is carried through: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_git_entry_that_is_neither_a_directory_nor_a_regular_file_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;
    let elsewhere = dir.path().join("elsewhere.git");
    fs::create_dir_all(&elsewhere).required()?;
    // symlinkは追跡せず、`.git`そのものの種別で判断する。
    std::os::unix::fs::symlink(&elsewhere, paths.host_clone().join(".git")).required()?;

    let error = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .refused_because("a linked git entry is not a worktree file")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert_eq!(reason_of(&error)?, "cause-not-a-regular-file");
    assert_eq!(
        shown_path(&error)?,
        paths::display(&paths.host_clone().join(".git"))
    );
    Ok(())
}

#[test]
fn a_git_file_that_names_no_git_directory_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;
    fs::write(paths.host_clone().join(".git"), "gitdir\nnothing here\n").required()?;

    let error = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .refused_because("a worktree file has to name a git directory")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert_eq!(reason_of(&error)?, "cause-no-git-directory-named");
    Ok(())
}

#[test]
fn a_relative_git_directory_is_resolved_against_the_clone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;

    // 案件directoryの中へ降りる相対pathは、案件の成果物として受け入れる。
    fs::write(
        paths.host_clone().join(".git"),
        "gitdir: ../.sbxm/worktree.git\n",
    )
    .required()?;
    let clone = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .required_because("a relative git directory inside the project is part of it")?;
    assert!(!clone.created, "the existing clone is reused");

    // 案件directoryを抜ける相対pathは、抜けた先を示して拒否する。
    fs::write(
        paths.host_clone().join(".git"),
        "gitdir: ../../elsewhere.git\n",
    )
    .required()?;
    let error = HostClone::ensure(
        &healthy(&paths.host_clone()),
        &paths,
        &repository,
        &mut SilentProgress,
    )
    .refused_because("a relative git directory that leaves the project is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    let diagnostic = &error.diagnostics()[0];
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { value, .. }
                if value.id == "cause-git-directory-outside"
                    && value.args.contains(&(
                        "observed",
                        paths::display(&dir.path().join("elsewhere.git"))
                    ))
                    && value.args.contains(&("root", paths::display(paths.root())))
        )),
        "the resolved path and the root it left are both named: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_clone_path_that_cannot_be_observed_is_never_cloned_over() -> Checked {
    if rustix::process::geteuid().is_root() {
        // rootはsearch bitに関わらず観測できるため、この状態を作れない。
        return Ok(());
    }
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());

    // 案件rootのsearch bitを外すと、その下にcloneがあるかを観測できなくなる。
    fs::set_permissions(paths.root(), fs::Permissions::from_mode(0o600)).required()?;
    let outcome = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress);
    fs::set_permissions(paths.root(), fs::Permissions::from_mode(0o700)).required()?;

    let error = outcome.refused_because("an unobservable clone path is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    assert!(
        host.args_of_calls().is_empty(),
        "a path sbxm could not observe is never cloned onto: {:?}",
        host.args_of_calls()
    );
    Ok(())
}

/// 起動された指定のうち、先頭の2語。どの検査まで進んだかを見る。
fn steps_of(host: &FakeGit) -> Vec<String> {
    host.args_of_calls()
        .iter()
        .map(|args| args.iter().take(2).cloned().collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn a_fresh_clone_that_fails_the_checks_is_refused_and_left_where_it_was_made() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let host = healthy(&paths.host_clone())
        .answering("rev-parse --is-bare-repository", "true\n")
        .cloning_into(&paths.host_clone());

    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a fresh clone is held to the rules a reused one is")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert_eq!(reason_of(&error)?, "cause-bare-repository");
    assert_eq!(
        steps_of(&host),
        ["clone --progress", "rev-parse --is-bare-repository"],
        "the clone is checked right after it is made"
    );
    assert!(
        paths.host_clone().join(".git").is_dir(),
        "what the clone made is left for the user to look at"
    );
    Ok(())
}

#[test]
fn a_clone_git_cannot_be_started_for_makes_nothing_and_checks_nothing() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    let host = healthy(&paths.host_clone())
        .cloning_into(&paths.host_clone())
        .unrunnable("clone");

    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a clone that never ran is not a clone")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(steps_of(&host), ["clone --progress"]);
    assert!(!paths.host_clone().exists());
    Ok(())
}

#[test]
fn a_clone_that_reports_success_but_left_nothing_is_refused_by_what_was_observed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    // gitは成功と答えたが、そこには何も無い。
    let host = healthy(&paths.host_clone());

    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("an absent working tree is not verified")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    assert_eq!(shown_path(&error)?, paths::display(&paths.host_clone()));
    assert!(
        error.diagnostics()[0].facts.iter().any(|fact| matches!(
            fact,
            Fact::OneLine { label, value }
                if label.id == "diagnostic-cause-label" && value.as_str().contains("os error")
        )),
        "the operating system's own wording is carried through: {:?}",
        error.diagnostics()[0].facts
    );
    assert_eq!(
        steps_of(&host),
        ["clone --progress"],
        "nothing is asked of a clone that is not there"
    );
    Ok(())
}

#[test]
fn a_check_git_cannot_answer_stops_the_reuse_at_that_check() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;

    for check in [
        "rev-parse --is-bare-repository",
        "rev-parse --show-toplevel",
        "config --get-all remote.origin.url",
    ] {
        let host = healthy(&paths.host_clone()).failing(check);
        let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
            .refused_because(&format!("{check} has no answer"))?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandFailed),
            "{check}: the failure of git is reported rather than read as an answer"
        );
        assert_eq!(
            host.args_of_calls().last().map(|args| args.join(" ")),
            Some(check.to_string()),
            "no check runs after the one that failed"
        );
        assert!(
            paths.host_clone().join(".git").is_dir(),
            "a clone that could not be checked is not removed"
        );
    }
    Ok(())
}

#[test]
fn a_check_git_cannot_be_started_for_is_reported_as_it_happened() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone().join(".git")).required()?;
    let host = healthy(&paths.host_clone()).unrunnable("rev-parse --is-bare-repository");

    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a check that never ran proves nothing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(steps_of(&host), ["rev-parse --is-bare-repository"]);
    Ok(())
}

#[test]
fn a_git_file_that_is_not_text_names_the_git_path_it_could_not_read() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (paths, repository) = project_paths(dir.path())?;
    fs::create_dir_all(paths.host_clone()).required()?;
    fs::write(paths.host_clone().join(".git"), [0xff, 0xfe, b'\n']).required()?;

    let host = healthy(&paths.host_clone());
    let error = HostClone::ensure(&host, &paths, &repository, &mut SilentProgress)
        .refused_because("a worktree file that cannot be read names no git directory")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    assert_eq!(
        shown_path(&error)?,
        paths::display(&paths.host_clone().join(".git"))
    );
    assert_eq!(
        remediation_path(&error)?,
        paths::display(&paths.host_clone())
    );
    assert!(
        error.diagnostics()[0].facts.iter().any(|fact| matches!(
            fact,
            Fact::OneLine { label, value }
                if label.id == "diagnostic-cause-label" && value.as_str().contains("UTF-8")
        )),
        "why it could not be read is carried through: {:?}",
        error.diagnostics()[0].facts
    );
    assert!(host.args_of_calls().is_empty(), "git never runs");
    Ok(())
}
