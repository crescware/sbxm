//! Host clone。
//!
//! 案件directory直下へrepositoryをcloneし、editorとhost上の操作から使えるようにする。
//! 既存cloneは、それが対象repositoryのcloneであると証明できた場合だけ再利用する。
//! dirty状態は構築の継続を妨げない。

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::git;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::ProjectId;

/// 採用したhost clone。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClone {
    pub path: PathBuf,
    /// この実行でcloneしたか。既存cloneを再利用した場合は`false`。
    pub created: bool,
}

/// host cloneを用意する。
///
/// GitHubへのSSH認証とrepository accessは、owner、repository、利用者のSSH設定に
/// よって結果が変わる。この工程のcloneを疎通の正本の検査とする。
pub fn ensure(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    project: &ProjectId,
) -> Result<HostClone> {
    let target = paths.host_clone();

    if fs::symlink_metadata(&target).is_err() {
        clone(host, &target, project)?;
        // 作成直後も、再利用と同じ規則で成果物を検証する。
        inspect(host, paths, project, &target)?;
        return Ok(HostClone {
            path: target,
            created: true,
        });
    }

    inspect(host, paths, project, &target)?;
    Ok(HostClone {
        path: target,
        created: false,
    })
}

fn clone(host: &dyn HostEnvironment, target: &Path, project: &ProjectId) -> Result<()> {
    let url = git::ssh_remote_url(project.owner(), project.repository());
    crate::progress::step(&msg!("progress-cloning-host"));
    // 進捗はgitが出したまま転送する。sbxmは実況を重ねない。
    let spec = CommandSpec::passthrough("git", &["clone", &url, &paths::display(target)])
        .timeout(TimeoutClass::RepositoryTransfer);
    host.run(&spec)?.require_success()?;
    Ok(())
}

/// 既存cloneが対象repositoryのものであることを確認する。
fn inspect(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    project: &ProjectId,
    target: &Path,
) -> Result<()> {
    if paths::is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(target)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(target, &error.to_string()))?;
    if !metadata.is_dir() {
        return Err(unusable(
            target,
            format!("the clone path is a {}", file_type_name(&metadata)),
        ));
    }
    require_git_dir_inside_the_project(paths, target)?;

    if read_git(host, target, &["rev-parse", "--is-bare-repository"])? != "false" {
        return Err(unusable(
            target,
            "the repository is bare, so it has no working tree".to_string(),
        ));
    }

    let top_level = read_git(host, target, &["rev-parse", "--show-toplevel"])?;
    let observed = canonicalize(Path::new(&top_level));
    let expected = canonicalize(target);
    if observed != expected {
        return Err(unusable(
            target,
            format!(
                "the working tree of {} is {}",
                paths::display(&expected),
                paths::display(&observed)
            ),
        ));
    }

    let urls = read_git(host, target, &["config", "--get-all", "remote.origin.url"])?;
    let urls: Vec<&str> = urls
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let [url] = urls.as_slice() else {
        return Err(unusable(
            target,
            format!("origin has {} URLs, so the remote is ambiguous", urls.len()),
        ));
    };
    let canonical = project.canonical();
    match git::canonical_id_of_remote(url) {
        Some(observed) if observed == canonical.as_str() => Ok(()),
        Some(observed) => Err(unusable(
            target,
            format!("origin points at {observed}, not at {canonical}"),
        )),
        None => Err(unusable(
            target,
            format!("origin URL {url} does not name a GitHub repository"),
        )),
    }
}

/// `.git`が案件directoryの外を指していないことを確認する。
///
/// worktree fileは`gitdir: <path>`の1行を持つ。指す先がproject root外にある場合、
/// 案件の成果物としては扱えない。
fn require_git_dir_inside_the_project(paths: &ProjectPaths, target: &Path) -> Result<()> {
    let git_path = target.join(".git");
    let metadata = fs::symlink_metadata(&git_path).map_err(|error| {
        unusable(
            target,
            format!("{} could not be read: {error}", paths::display(&git_path)),
        )
    })?;

    if metadata.is_dir() {
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(unusable(
            target,
            format!(
                "{} is a {}",
                paths::display(&git_path),
                file_type_name(&metadata)
            ),
        ));
    }

    let contents = fs::read_to_string(&git_path).map_err(|error| {
        unusable(
            target,
            format!("{} could not be read: {error}", paths::display(&git_path)),
        )
    })?;
    let Some(declared) = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))
    else {
        return Err(unusable(
            target,
            format!("{} names no git directory", paths::display(&git_path)),
        ));
    };
    let declared = declared.trim();
    let resolved = if Path::new(declared).is_absolute() {
        paths::lexically_standardize(Path::new(declared))
    } else {
        paths::lexically_standardize(&target.join(declared))
    };
    if !resolved.starts_with(paths.root()) {
        return Err(unusable(
            target,
            format!(
                "{} points at {}, which is outside {}",
                paths::display(&git_path),
                paths::display(&resolved),
                paths::display(paths.root())
            ),
        ));
    }
    Ok(())
}

/// 作業directoryを対象cloneに固定してgitの結果を読む。
fn read_git(host: &dyn HostEnvironment, target: &Path, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::capture("git", args)
        .timeout(TimeoutClass::LocalFilesystem)
        .working_dir(target);
    let outcome = host.run(&spec)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
fn canonicalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| paths::lexically_standardize(path))
}

fn file_type_name(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else {
        "special file"
    }
}

/// 既存cloneを自動削除せず、観測した不一致を示して停止する。
fn unusable(target: &Path, detail: String) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::HostCloneUnusable,
            msg!(
                "error-host-clone-unusable",
                path = paths::display(target),
                detail = detail
            ),
        )
        .remediation(msg!(
            "remediation-host-clone-unusable",
            path = paths::display(target)
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, HostEnvironment};
    use crate::paths::AbsoluteBasePath;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;

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
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn project_paths(dir: &Path) -> (ProjectPaths, ProjectId) {
        let base = AbsoluteBasePath::new(dir).expect("valid base path");
        let project = ProjectId::parse("Example-Org/Example-Repo").expect("valid project id");
        let paths = ProjectPaths::derive(&base, &project.canonical());
        fs::create_dir_all(paths.root()).expect("create the project root");
        (paths, project)
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
        let (paths, project) = project_paths(dir.path());
        let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());

        let clone = ensure(&host, &paths, &project).expect("the clone is created");
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
        let (paths, project) = project_paths(dir.path());
        let host = healthy(&paths.host_clone()).cloning_into(&paths.host_clone());
        ensure(&host, &paths, &project).expect("the clone is created");

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
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
        let host = healthy(&paths.host_clone());

        let clone = ensure(&host, &paths, &project).expect("the existing clone is reused");
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
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
        fs::write(paths.host_clone().join("uncommitted.txt"), b"work").unwrap();

        ensure(&healthy(&paths.host_clone()), &paths, &project)
            .expect("uncommitted work is the user's, not a reason to stop");
    }

    #[test]
    fn a_clone_of_another_repository_is_refused_instead_of_being_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone().join(".git")).unwrap();
        let host = healthy(&paths.host_clone()).answering(
            "config --get-all remote.origin.url",
            "git@github.com:other-org/other-repo.git\n",
        );

        let error = ensure(&host, &paths, &project).expect_err("a different remote is refused");
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
        assert!(
            paths.host_clone().join(".git").exists(),
            "nothing is deleted when the clone cannot be used"
        );
    }

    #[test]
    fn an_ambiguous_or_missing_origin_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

        for urls in [
            "",
            "git@github.com:Example-Org/Example-Repo.git\nhttps://github.com/example-org/example-repo.git\n",
        ] {
            let host =
                healthy(&paths.host_clone()).answering("config --get-all remote.origin.url", urls);
            let error =
                ensure(&host, &paths, &project).expect_err("origin must name exactly one remote");
            assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
        }
    }

    #[test]
    fn a_bare_repository_or_a_nested_working_tree_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone().join(".git")).unwrap();

        let bare =
            healthy(&paths.host_clone()).answering("rev-parse --is-bare-repository", "true\n");
        let error = ensure(&bare, &paths, &project).expect_err("a bare repository has no worktree");
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));

        // 期待pathの中に居ても、top-levelが別のdirectoryなら再利用しない。
        let nested = healthy(&paths.host_clone()).answering(
            "rev-parse --show-toplevel",
            &format!("{}\n", dir.path().display()),
        );
        let error =
            ensure(&nested, &paths, &project).expect_err("the clone must be its own working tree");
        assert_eq!(error.first_id(), Some(ErrorId::HostCloneUnusable));
    }

    #[test]
    fn a_git_directory_that_points_outside_the_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, project) = project_paths(dir.path());
        fs::create_dir_all(paths.host_clone()).unwrap();
        let elsewhere = dir.path().join("elsewhere.git");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(
            paths.host_clone().join(".git"),
            format!("gitdir: {}\n", elsewhere.display()),
        )
        .unwrap();

        let error = ensure(&healthy(&paths.host_clone()), &paths, &project)
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
        ensure(&healthy(&paths.host_clone()), &paths, &project)
            .expect("a git directory inside the project is part of the project");
    }
}
