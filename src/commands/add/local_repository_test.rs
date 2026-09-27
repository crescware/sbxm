use std::path::{Path, PathBuf};

use crate::boundary::host::RealHost;
use crate::diagnostics::ErrorId;
use crate::paths::ProjectParent;
use crate::repository::Provider;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::git_in;

use super::LocalRepository;

/// commitを1つ持つworking treeを`parent/<name>`に作り、その実pathを返す。
fn work_tree(parent: &Path, name: &str) -> Checked<PathBuf> {
    let path = parent.join(name);
    std::fs::create_dir_all(&path).required()?;
    git_in(&path, &["init", "--quiet"])?;
    git_in(
        &path,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;
    std::fs::canonicalize(&path).required()
}

fn resolve(
    parent: &Path,
    path: &Path,
    name: Option<&str>,
) -> crate::diagnostics::Result<LocalRepository> {
    let parent = ProjectParent::at(parent)?;
    LocalRepository::resolve(&RealHost, &parent, path, name)
}

fn text(path: &Path) -> Checked<&str> {
    path.to_str().required_because("a UTF-8 path")
}

#[test]
fn a_git_directory_is_registered_at_its_real_path_on_its_current_branch() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "Example-Repo")?;

    // 相対pathはcwdから解決する。名前は`git clone`と同じく`.git`の上のdirectoryから取る。
    let resolved = resolve(dir.path(), Path::new("Example-Repo/.git"), None).required()?;

    assert_eq!(resolved.identity.provider(), Provider::Local);
    assert_eq!(resolved.identity.display_id(), "local/Example-Repo");
    assert_eq!(
        resolved.identity.clone_url(),
        text(&repository.join(".git"))?
    );
    assert_eq!(resolved.branch.as_deref(), Some("main"));
    Ok(())
}

#[test]
fn a_symlink_is_registered_as_the_repository_it_points_at() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(repository.join(".git"), &link).required()?;

    let resolved = resolve(dir.path(), &link, None).required()?;

    assert_eq!(
        resolved.identity.clone_url(),
        text(&repository.join(".git"))?
    );
    assert_eq!(resolved.identity.display_id(), "local/app");
    Ok(())
}

#[test]
fn a_bare_repository_is_registered_under_the_name_git_clone_would_give_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = work_tree(dir.path(), "source")?;
    let bare = dir.path().join("app.git");
    git_in(
        dir.path(),
        &["clone", "--quiet", "--bare", text(&source)?, text(&bare)?],
    )?;

    let resolved = resolve(dir.path(), &bare, None).required()?;

    assert_eq!(
        resolved.identity.clone_url(),
        text(&std::fs::canonicalize(&bare).required()?)?
    );
    assert_eq!(resolved.identity.display_id(), "local/app");
    assert_eq!(resolved.branch.as_deref(), Some("main"));
    Ok(())
}

#[test]
fn a_worktree_git_file_registers_the_shared_repository_on_the_worktree_branch() -> Checked {
    // `.git` fileはrepositoryを1つだけ指す。記録するのはworktreeが共有するrepositoryで
    // あり、起点はそのworktreeが今いるbranchである。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    let worktree = dir.path().join("app-feature");
    git_in(
        &repository,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feature",
            text(&worktree)?,
        ],
    )?;

    let resolved = resolve(dir.path(), &worktree.join(".git"), None).required()?;

    assert_eq!(
        resolved.identity.clone_url(),
        text(&repository.join(".git"))?
    );
    assert_eq!(resolved.identity.display_id(), "local/app");
    assert_eq!(resolved.branch.as_deref(), Some("feature"));
    Ok(())
}

#[test]
fn a_detached_repository_has_no_branch_to_start_from() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    git_in(&repository, &["checkout", "--quiet", "--detach"])?;

    let resolved = resolve(dir.path(), &repository.join(".git"), None).required()?;

    assert_eq!(resolved.branch, None);
    Ok(())
}

#[test]
fn a_directory_name_that_cannot_name_a_project_asks_for_a_name() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let git_dir = work_tree(dir.path(), "my app")?.join(".git");

    let error = resolve(dir.path(), &git_dir, None).refused_because("a space")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalNameUnusable));

    let named = resolve(dir.path(), &git_dir, Some("tool")).required()?;
    assert_eq!(named.identity.display_id(), "local/tool");

    let error = resolve(dir.path(), &git_dir, Some("not valid")).refused_because("a space")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidProjectId));
    Ok(())
}

#[test]
fn a_path_that_is_not_a_git_directory_is_refused() -> Checked {
    // working treeのdirectoryは、その中のどのrepositoryを指したのかが決まらない。
    // 上のdirectoryのrepositoryも探さない。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    std::fs::create_dir_all(repository.join("src")).required()?;
    let plain = dir.path().join("plain");
    std::fs::create_dir_all(&plain).required()?;
    let file = dir.path().join("file");
    std::fs::write(&file, "not a repository\n").required()?;

    for path in [
        repository.clone(),
        repository.join("src"),
        plain,
        file,
        dir.path().join("missing"),
    ] {
        let error = resolve(dir.path(), &path, None).refused_because("not a git directory")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::LocalRepositoryUnusable),
            "{}",
            path.display()
        );
    }
    Ok(())
}

#[test]
fn a_branch_that_shares_its_name_with_a_tag_is_recorded_by_its_branch_name() -> Checked {
    // 短い名前が曖昧なとき、gitは`heads/main`と答える。そのまま記録すると、originに
    // `heads/main`というbranchを探して構築が失敗する。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    git_in(&repository, &["tag", "main"])?;

    let resolved = resolve(dir.path(), &repository.join(".git"), None).required()?;

    assert_eq!(resolved.branch.as_deref(), Some("main"));
    Ok(())
}

#[test]
fn a_parent_inside_any_working_tree_of_the_repository_is_found() -> Checked {
    // 案件directoryを作る場所から上へgitに探させる。本体のworking tree、worktree、
    // git directoryのどこにいても、同じrepositoryが見つかる。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    std::fs::create_dir_all(repository.join("src")).required()?;
    let worktree = dir.path().join("app-feature");
    git_in(
        &repository,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feature",
            text(&worktree)?,
        ],
    )?;
    let git_dir = repository.join(".git");

    for parent in [
        repository.clone(),
        repository.join("src"),
        git_dir.clone(),
        worktree,
    ] {
        let resolved = resolve(&parent, &git_dir, None).required()?;
        assert!(resolved.encloses_parent, "{}", parent.display());
    }
    Ok(())
}

#[test]
fn a_parent_outside_the_repository_is_not_inside_it() -> Checked {
    // 中に別のrepositoryを置いたdirectoryは、登録するrepositoryの中ではない。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    let nested = work_tree(&repository, "vendored")?;
    let git_dir = repository.join(".git");

    for parent in [dir.path().to_path_buf(), nested] {
        let resolved = resolve(&parent, &git_dir, None).required()?;
        assert!(!resolved.encloses_parent, "{}", parent.display());
    }
    Ok(())
}

// --- gitを台本で答えさせる経路 ---
//
// 実物のgitでは作れない答え、つまり起動の失敗と想定外の終了statusを、台本のhostで返す。

/// 台本のgitが1つの問いへ返すもの。
enum Answer {
    Exits(i32, String),
    Unrunnable,
}

/// `--git-common-dir`、`symbolic-ref`、`-C <parent>`の3つの問いへ台本で答えるhost。
struct ScriptedGit {
    common_dir: Answer,
    branch: Answer,
    enclosing: Answer,
    calls: std::cell::RefCell<Vec<String>>,
}

impl ScriptedGit {
    /// `git_dir`をそのままrepositoryとし、`main`の上にいて、親はその外にあると答える。
    fn answering_for(git_dir: &Path) -> ScriptedGit {
        ScriptedGit {
            common_dir: Answer::Exits(0, format!("{}\n", git_dir.display())),
            branch: Answer::Exits(0, "refs/heads/main\n".to_string()),
            enclosing: Answer::Exits(128, String::new()),
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 答えた問いの名前。
    fn asked(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl crate::boundary::host::HostEnvironment for ScriptedGit {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(
        &self,
        spec: &crate::boundary::host::CommandSpec,
    ) -> crate::diagnostics::Result<crate::boundary::host::CommandOutcome> {
        let (name, answer) = if spec.args.first().is_some_and(|arg| arg == "-C") {
            ("enclosing", &self.enclosing)
        } else if spec.args.iter().any(|arg| arg == "symbolic-ref") {
            ("branch", &self.branch)
        } else {
            ("common-dir", &self.common_dir)
        };
        self.calls.borrow_mut().push(name.to_string());
        match answer {
            Answer::Exits(code, stdout) => {
                Ok(crate::testing::command::outcome(spec, *code, stdout))
            }
            Answer::Unrunnable => Err(crate::diagnostics::Error::new(
                ErrorId::ExternalCommandSpawnFailed,
                crate::msg!("error-external-command-spawn-failed"),
            )),
        }
    }
}

/// 1つの問いを起動できなくする手続き。
type MakeUnrunnable = fn(&mut ScriptedGit);

fn resolve_with(
    host: &ScriptedGit,
    parent: &Path,
    path: &Path,
) -> crate::diagnostics::Result<LocalRepository> {
    let parent = ProjectParent::at(parent)?;
    LocalRepository::resolve(host, &parent, path, None)
}

/// 観測した理由として示したmessage ID。
fn reason_of(error: &crate::diagnostics::Error) -> Checked<&'static str> {
    error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            crate::design::Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the refusal names what it observed")
}

/// UTF-8にならない名前のdirectoryを`parent`の下に作る。作れないfilesystemでは`None`。
fn directory_not_named_in_utf8(parent: &Path, suffix: &str) -> Checked<Option<PathBuf>> {
    use std::os::unix::ffi::OsStrExt;

    // 有効なUTF-8にはならない、単独の続きbyte。
    let mut name = vec![0x80, 0x81];
    name.extend_from_slice(suffix.as_bytes());
    let path = parent.join(std::ffi::OsStr::from_bytes(&name));
    let created = std::fs::create_dir_all(&path);
    if created
        .as_ref()
        .err()
        .and_then(std::io::Error::raw_os_error)
        == Some(rustix::io::Errno::ILSEQ.raw_os_error())
    {
        // macOSのAPFSは不正UTF-8のfile name自体を作成できない。
        return Ok(None);
    }
    created.required()?;
    Ok(Some(path))
}

#[test]
fn a_git_directory_whose_real_path_is_not_utf8_is_refused_before_git_runs() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let Some(repository) = directory_not_named_in_utf8(dir.path(), "app")? else {
        return Ok(());
    };
    let git_dir = repository.join(".git");
    std::fs::create_dir_all(&git_dir).required()?;
    let host = ScriptedGit::answering_for(&git_dir);

    let error = resolve_with(&host, dir.path(), &git_dir)
        .refused_because("a path that cannot be recorded or handed to git")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRepositoryUnusable));
    assert_eq!(reason_of(&error)?, "cause-path-not-utf8");
    assert!(
        host.asked().is_empty(),
        "git never runs: {:?}",
        host.asked()
    );
    Ok(())
}

#[test]
fn a_shared_repository_whose_real_path_is_not_utf8_is_refused() -> Checked {
    // 渡したgit directoryはUTF-8でも、worktreeが共有するrepositoryの実体がそうでない。
    let dir = tempfile::tempdir().required()?;
    let Some(shared) = directory_not_named_in_utf8(dir.path(), "shared.git")? else {
        return Ok(());
    };
    let link = dir.path().join("shared-link.git");
    std::os::unix::fs::symlink(&shared, &link).required()?;
    let git_dir = dir.path().join("worktree.git");
    std::fs::create_dir_all(&git_dir).required()?;
    let mut host = ScriptedGit::answering_for(&git_dir);
    host.common_dir = Answer::Exits(0, format!("{}\n", link.display()));

    let error = resolve_with(&host, dir.path(), &git_dir)
        .refused_because("the repository to record cannot be written down")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalRepositoryUnusable));
    assert_eq!(reason_of(&error)?, "cause-path-not-utf8");
    assert_eq!(
        host.asked(),
        ["common-dir"],
        "nothing more is asked about a repository that cannot be recorded"
    );
    Ok(())
}

#[test]
fn a_git_that_cannot_be_started_stops_the_resolution_at_that_question() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let git_dir = dir.path().join("app").join(".git");
    std::fs::create_dir_all(&git_dir).required()?;
    let git_dir = std::fs::canonicalize(&git_dir).required()?;

    // 問いは順に訊く。起動できなかった問いのあとは何も訊かない。
    let questions: [(&str, MakeUnrunnable); 3] = [
        ("common-dir", |host| host.common_dir = Answer::Unrunnable),
        ("branch", |host| host.branch = Answer::Unrunnable),
        ("enclosing", |host| host.enclosing = Answer::Unrunnable),
    ];
    for (index, (question, unrunnable)) in questions.iter().enumerate() {
        let mut host = ScriptedGit::answering_for(&git_dir);
        unrunnable(&mut host);

        let error = resolve_with(&host, dir.path(), &git_dir)
            .refused_because(&format!("{question} could not be asked"))?;
        // gitを起動できなかったことは、git directoryでないこととして言い換えない。
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandSpawnFailed),
            "{question}"
        );
        let asked: Vec<&str> = questions[..=index].iter().map(|(name, _)| *name).collect();
        assert_eq!(host.asked(), asked, "{question}");
    }
    Ok(())
}

#[test]
fn a_head_git_cannot_read_is_a_failure_rather_than_a_detached_head() -> Checked {
    // 終了status 1だけがdetached HEADである。それ以外の失敗から起点を無いとは読まない。
    let dir = tempfile::tempdir().required()?;
    let git_dir = dir.path().join("app").join(".git");
    std::fs::create_dir_all(&git_dir).required()?;
    let git_dir = std::fs::canonicalize(&git_dir).required()?;
    let mut host = ScriptedGit::answering_for(&git_dir);
    host.branch = Answer::Exits(128, String::new());

    let error = resolve_with(&host, dir.path(), &git_dir)
        .refused_because("an unreadable HEAD has no known branch")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert_eq!(host.asked(), ["common-dir", "branch"]);

    // 同じhostでも、1で答えればdetachedとして続ける。
    host.branch = Answer::Exits(1, String::new());
    let resolved = resolve_with(&host, dir.path(), &git_dir).required()?;
    assert_eq!(resolved.branch, None);
    Ok(())
}
