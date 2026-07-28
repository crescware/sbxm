//! Sandbox内のbare repositoryとmanaged worktree。
//!
//! 1 Sandboxにつき1つのbare repositoryを持ち、作業用のworktreeをその下に並べる。
//! 1 treeの場合もbare repositoryとworktreeを分離する。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::git;
use crate::metadata::{self, CreationMode, ManagedWorktree, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout};

use super::sandbox;

/// このbuildが使うfetch refspec。完全一致だけを再利用の条件とする。
const FETCH_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";

/// bare repositoryを用意する。
///
/// 既存のdirectoryは、対象repositoryのbare cloneであると証明できた場合だけ再利用し、
/// 条件を満たさない場合は自動削除せずに停止する。
pub fn ensure_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    layout: &SandboxLayout,
) -> Result<()> {
    let git_dir = layout.bare_git_dir();

    if sandbox::path_exists(host, sandbox, &git_dir)? {
        verify_bare_clone(host, sandbox, project, &git_dir)?;
    } else {
        sandbox::exec(host, sandbox, &["mkdir", "-p", &layout.bare_root()])?.require_success()?;
        let url = git::https_remote_url(project.owner(), project.repository());
        // `git clone --bare`はremoteのbranchを`refs/heads/*`へ複製する。そのbranchは
        // worktreeを作るときに同じ名前で作ろうとするものと衝突する。bare repositoryは
        // remote-tracking refだけを持つ入れ物として始める。
        sandbox::exec(host, sandbox, &["git", "init", "--bare", &git_dir])?.require_success()?;
        sandbox::exec(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                &git_dir,
                "remote",
                "add",
                "origin",
                &url,
            ],
        )?
        .require_success()?;
        sandbox::exec(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                &git_dir,
                "config",
                "remote.origin.fetch",
                FETCH_REFSPEC,
            ],
        )?
        .require_success()?;
        verify_bare_clone(host, sandbox, project, &git_dir)?;
    }

    // remote-tracking refを現在の状態にしてから、起点refを解決する。
    sandbox::exec_with_progress(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "fetch", "--prune", "origin"],
    )?
    .require_success()?;
    Ok(())
}

fn verify_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    git_dir: &str,
) -> Result<()> {
    let bare = read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    )?;
    if bare != "true" {
        return Err(unusable(
            git_dir,
            "the repository is not bare, so it is not the shared repository".to_string(),
        ));
    }

    let urls = read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.url",
        ],
    )?;
    let urls: Vec<&str> = urls
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let [url] = urls.as_slice() else {
        return Err(unusable(
            git_dir,
            format!("origin has {} URLs, so the remote is ambiguous", urls.len()),
        ));
    };
    let canonical = project.canonical();
    match git::canonical_id_of_remote(url) {
        Some(observed) if observed == canonical.as_str() => {}
        Some(observed) => {
            return Err(unusable(
                git_dir,
                format!("origin points at {observed}, not at {canonical}"),
            ));
        }
        None => {
            return Err(unusable(
                git_dir,
                format!("origin URL {url} does not name a GitHub repository"),
            ));
        }
    }

    let refspecs = read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.fetch",
        ],
    )?;
    let refspecs: Vec<&str> = refspecs
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if refspecs != [FETCH_REFSPEC] {
        return Err(unusable(
            git_dir,
            format!(
                "the fetch refspec is {}, not {FETCH_REFSPEC}",
                refspecs.join(", ")
            ),
        ));
    }

    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "--git-dir", git_dir, "fsck", "--connectivity-only"],
    )?;
    if !outcome.success() {
        return Err(unusable(
            git_dir,
            "the repository does not pass a connectivity check".to_string(),
        ));
    }
    Ok(())
}

/// 起点となるbranchを確定させる。
///
/// hostのvalidationは、外部commandへ渡す前に確実に拒否できる条件だけを見る。
/// 起点として使う名前は、Sandbox内のgit自身にもう一度判定させてから採用する。
/// attached modeでremote default branchを解決した場合は、その場でmetadataへ記録する。
pub fn resolve_start_ref(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    paths: &ProjectPaths,
    project: &mut ProjectMetadata,
) -> Result<String> {
    let git_dir = layout.bare_git_dir();

    let stored = project.provisioning.start_ref.clone();
    let branch = match &stored {
        Some(branch) => branch.clone(),
        None => remote_default_branch(host, sandbox, &git_dir)?,
    };
    require_branch_name(host, sandbox, &branch)?;
    if stored.is_none() {
        project.provisioning.start_ref = Some(branch.clone());
        metadata::update(paths, project)?;
    }

    // tagやambiguous refを起点にしないよう、完全なremote-tracking refだけを確認する。
    let reference = git::origin_ref(&branch);
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "show-ref",
            "--verify",
            "--quiet",
            &reference,
        ],
    )?;
    if !outcome.success() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::StartRefUnresolved,
                msg!(
                    "error-start-ref-unresolved",
                    reference = reference,
                    project = project.display_id()
                ),
            )
            .remediation(msg!("remediation-start-ref-unresolved")),
        ));
    }
    Ok(branch)
}

/// 起点branch名を、Sandbox内の`git check-ref-format --branch`で再検証する。
///
/// repositoryを指定せずに実行するため、`@{-1}`のような文脈依存の短縮形は展開されず、
/// branch名としてそのまま判定される。
fn require_branch_name(host: &dyn HostEnvironment, sandbox: &str, branch: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "check-ref-format", "--branch", branch],
    )?;
    if outcome.success() {
        return Ok(());
    }
    fail(
        ErrorId::InvalidBranchName,
        msg!(
            "error-invalid-branch-name",
            value = branch,
            detail = "git in the sandbox does not accept this as a branch name"
        ),
    )
}

/// `git ls-remote --symref origin HEAD`が示すdefault branch。
fn remote_default_branch(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
) -> Result<String> {
    let output = read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "ls-remote",
            "--symref",
            "origin",
            "HEAD",
        ],
    )?;

    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("ref:") else {
            continue;
        };
        let Some(reference) = rest.split_whitespace().next() else {
            continue;
        };
        // branch以外がHEADの指す先である場合は受け付けない。
        if let Some(branch) = reference.strip_prefix("refs/heads/")
            && git::validate_branch_name(branch).is_ok()
        {
            return Ok(branch.to_string());
        }
    }

    Err(Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = "git ls-remote --symref",
            detail = "no branch was reported for HEAD"
        ),
    ))
}

/// managed worktreeを、indexを固定したまま用意する。
///
/// 作成のたびにmetadataへ1件ずつ追記するため、途中で中断しても次の実行が続きから進む。
pub fn ensure_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    paths: &ProjectPaths,
    project: &mut ProjectMetadata,
    branch: &str,
) -> Result<Vec<ManagedWorktree>> {
    let git_dir = layout.bare_git_dir();
    let reference = git::origin_ref(branch);
    let expected_commit = read(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "rev-parse", &reference],
    )?;
    let mode = project.provisioning.mode;

    let mut managed = Vec::new();
    for index in 0..project.provisioning.requested_worktrees {
        let name = layout.worktree_name(index);
        let path = layout.worktree(index);
        let recorded = project
            .managed_worktrees
            .iter()
            .any(|worktree| worktree.path == name);

        if !sandbox::path_exists(host, sandbox, &path)? {
            create_worktree(host, sandbox, &git_dir, &path, branch, mode)?;
        }
        verify_worktree(host, sandbox, &path, &expected_commit, branch, mode)?;

        let entry = ManagedWorktree {
            path: name,
            created_from: reference.clone(),
        };
        if !recorded {
            // 作成直後の1件だけをmetadataへ足す。
            project.managed_worktrees.push(entry.clone());
            metadata::update(paths, project)?;
        }
        managed.push(entry);
    }
    Ok(managed)
}

fn create_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let reference = git::origin_ref(branch);
    let arguments: Vec<&str> = match mode {
        CreationMode::Attached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--track",
            "-b",
            branch,
            path,
            &reference,
        ],
        CreationMode::Detached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--detach",
            path,
            &reference,
        ],
    };
    sandbox::exec(host, sandbox, &arguments)?.require_success()?;
    Ok(())
}

/// worktreeが期待するcommitとmodeであることを確認する。
fn verify_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    path: &str,
    expected_commit: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let head = read(host, sandbox, &["git", "-C", path, "rev-parse", "HEAD"])?;
    if head != expected_commit {
        return Err(unusable(
            path,
            format!("HEAD is {head}, and this project starts from {expected_commit}"),
        ));
    }

    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "-C", path, "symbolic-ref", "-q", "HEAD"],
    )?;
    let observed = outcome.stdout_text().trim().to_string();
    match mode {
        CreationMode::Attached => {
            let expected = format!("refs/heads/{branch}");
            if !outcome.success() || observed != expected {
                return Err(unusable(path, format!("the worktree is not on {expected}")));
            }
        }
        CreationMode::Detached => {
            if outcome.success() {
                return Err(unusable(
                    path,
                    format!("the worktree is on {observed}, and this project uses detached heads"),
                ));
            }
        }
    }
    Ok(())
}

/// Sandbox内のcommandの標準出力。
fn read(host: &dyn HostEnvironment, sandbox: &str, args: &[&str]) -> Result<String> {
    let outcome = sandbox::exec(host, sandbox, args)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}

/// 成果物を自動削除せず、観測した不一致を示して停止する。
fn unusable(path: &str, detail: String) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!(
                "error-sandbox-repository-unusable",
                path = path,
                detail = detail
            ),
        )
        .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, CommandSpec};
    use crate::metadata::Provisioning;
    use crate::paths::AbsoluteBasePath;
    use crate::project::CanonicalProjectId;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;

    const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

    struct FakeSandbox {
        /// Sandbox内に存在するpath。
        present: RefCell<Vec<String>>,
        /// 特定のcommandに対する応答。
        answers: HashMap<String, (i32, String)>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSandbox {
        fn new() -> FakeSandbox {
            FakeSandbox {
                present: RefCell::new(Vec::new()),
                answers: HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn holding(mut self, paths: &[&str]) -> FakeSandbox {
            self.present = RefCell::new(paths.iter().map(|path| path.to_string()).collect());
            self
        }

        fn answering(mut self, command: &str, stdout: &str) -> FakeSandbox {
            self.answers
                .insert(command.to_string(), (0, stdout.to_string()));
            self
        }

        fn failing(mut self, command: &str) -> FakeSandbox {
            self.answers.insert(command.to_string(), (1, String::new()));
            self
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        fn ran(&self, needle: &str) -> bool {
            self.calls()
                .iter()
                .any(|args| args.join(" ").contains(needle))
        }
    }

    impl HostEnvironment for FakeSandbox {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.args.clone());
            let inner: Vec<String> = spec
                .args
                .iter()
                .skip_while(|arg| *arg != "--")
                .skip(1)
                .cloned()
                .collect();
            let key = inner.join(" ");

            let (code, stdout) = if inner.first().is_some_and(|arg| arg == "test") {
                let target = inner.last().cloned().unwrap_or_default();
                (
                    i32::from(!self.present.borrow().contains(&target)),
                    String::new(),
                )
            } else if key.contains("worktree add") {
                // 作成に成功したworktreeは、以後存在するものとして扱う。
                let path = inner
                    .iter()
                    .find(|arg| arg.contains(".tree-"))
                    .cloned()
                    .unwrap_or_default();
                self.present.borrow_mut().push(path);
                (0, String::new())
            } else {
                match self.answers.get(&key) {
                    Some((code, stdout)) => (*code, stdout.clone()),
                    None => (0, String::new()),
                }
            };

            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

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
    fn healthy_clone() -> FakeSandbox {
        let git_dir = layout().bare_git_dir();
        FakeSandbox::new()
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
            managed_worktrees: Vec::new(),
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
        let host = FakeSandbox::new().answering(
            &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
            "ref: refs/heads/main\tHEAD\n9f5b1c\tHEAD\n",
        );

        let mut project = metadata(CreationMode::Attached, None, 1);
        metadata::create(&paths, &project).expect("write the metadata");

        let branch = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
            .expect("resolve");
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
    fn the_start_branch_is_judged_again_by_git_inside_the_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths(dir.path());
        // hostのvalidationは通るが、gitがbranch名として受け付けない値。
        let host = FakeSandbox::new().failing("git check-ref-format --branch feature..login");

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
        let host = FakeSandbox::new()
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
        let host = FakeSandbox::new().failing(&format!(
            "git --git-dir {git_dir} show-ref --verify --quiet refs/remotes/origin/develop"
        ));

        let mut project = metadata(CreationMode::Detached, Some("develop"), 1);
        metadata::create(&paths, &project).expect("write the metadata");

        let error = resolve_start_ref(&host, "sbxm-example", &layout(), &paths, &mut project)
            .expect_err("a branch that is not on the remote cannot be a start point");
        assert_eq!(error.first_id(), Some(ErrorId::StartRefUnresolved));
    }

    /// worktreeの検査を通る応答。
    fn worktree_host(mode: CreationMode, count: u32) -> FakeSandbox {
        let git_dir = layout().bare_git_dir();
        let mut host = FakeSandbox::new().answering(
            &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/develop"),
            &format!("{COMMIT}\n"),
        );
        for index in 0..count {
            let path = layout().worktree(index);
            host = host.answering(
                &format!("git -C {path} rev-parse HEAD"),
                &format!("{COMMIT}\n"),
            );
            host = match mode {
                CreationMode::Attached => host.answering(
                    &format!("git -C {path} symbolic-ref -q HEAD"),
                    "refs/heads/develop\n",
                ),
                CreationMode::Detached => {
                    host.failing(&format!("git -C {path} symbolic-ref -q HEAD"))
                }
            };
        }
        host
    }

    #[test]
    fn detached_worktrees_are_created_from_one_commit_and_recorded_one_by_one() {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths(dir.path());
        let host = worktree_host(CreationMode::Detached, 3);
        let mut project = metadata(CreationMode::Detached, Some("develop"), 3);
        metadata::create(&paths, &project).expect("write the metadata");

        let managed = ensure_worktrees(
            &host,
            "sbxm-example",
            &layout(),
            &paths,
            &mut project,
            "develop",
        )
        .expect("create");

        assert_eq!(
            managed
                .iter()
                .map(|worktree| worktree.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "example-repo.tree-0",
                "example-repo.tree-1",
                "example-repo.tree-2"
            ]
        );
        assert!(
            managed
                .iter()
                .all(|worktree| worktree.created_from == "refs/remotes/origin/develop")
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

        let stored = metadata::load(&paths).unwrap().expect("present");
        assert_eq!(stored.managed_worktrees, managed);
    }

    #[test]
    fn an_attached_project_gets_one_tracking_branch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths(dir.path());
        let host = worktree_host(CreationMode::Attached, 1);
        let mut project = metadata(CreationMode::Attached, Some("develop"), 1);
        metadata::create(&paths, &project).expect("write the metadata");

        ensure_worktrees(
            &host,
            "sbxm-example",
            &layout(),
            &paths,
            &mut project,
            "develop",
        )
        .expect("create");

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
        let mut project = metadata(CreationMode::Detached, Some("develop"), 1);
        metadata::create(&paths, &project).expect("write the metadata");

        let managed = ensure_worktrees(
            &host,
            "sbxm-example",
            &layout(),
            &paths,
            &mut project,
            "develop",
        )
        .expect("adopt");

        assert_eq!(managed.len(), 1);
        assert!(
            !host.ran("worktree add"),
            "an interrupted creation is adopted rather than repeated"
        );
        let stored = metadata::load(&paths).unwrap().expect("present");
        assert_eq!(stored.managed_worktrees.len(), 1);
    }

    #[test]
    fn a_worktree_on_another_commit_or_in_the_wrong_mode_stops_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths(dir.path());
        let path = layout().worktree(0);

        let elsewhere = worktree_host(CreationMode::Detached, 1)
            .holding(&[&path])
            .answering(&format!("git -C {path} rev-parse HEAD"), "0123456789\n");
        let attached_by_mistake = worktree_host(CreationMode::Detached, 1)
            .holding(&[&path])
            .answering(
                &format!("git -C {path} symbolic-ref -q HEAD"),
                "refs/heads/develop\n",
            );

        for host in [elsewhere, attached_by_mistake] {
            let mut project = metadata(CreationMode::Detached, Some("develop"), 1);
            metadata::create(&paths, &project).ok();
            let error = ensure_worktrees(
                &host,
                "sbxm-example",
                &layout(),
                &paths,
                &mut project,
                "develop",
            )
            .expect_err("a worktree that does not match is not adopted");
            assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
        }
    }
}
