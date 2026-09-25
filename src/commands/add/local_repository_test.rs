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
