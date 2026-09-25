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

#[test]
fn a_work_tree_is_registered_at_its_real_path_on_its_current_branch() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "Example-Repo")?;

    // 相対pathはcwdから解決する。
    let resolved = resolve(dir.path(), Path::new("Example-Repo"), None).required()?;

    assert_eq!(resolved.identity.provider(), Provider::Local);
    assert_eq!(resolved.identity.display_id(), "local/Example-Repo");
    assert_eq!(
        resolved.identity.clone_url(),
        repository.to_str().required()?
    );
    assert_eq!(resolved.branch.as_deref(), Some("main"));
    Ok(())
}

#[test]
fn a_symlink_is_registered_as_the_repository_it_points_at() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&repository, &link).required()?;

    let resolved = resolve(dir.path(), &link, None).required()?;

    assert_eq!(
        resolved.identity.clone_url(),
        repository.to_str().required()?
    );
    assert_eq!(resolved.identity.display_id(), "local/app");
    Ok(())
}

#[test]
fn a_detached_repository_has_no_branch_to_start_from() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    git_in(&repository, &["checkout", "--quiet", "--detach"])?;

    let resolved = resolve(dir.path(), &repository, None).required()?;

    assert_eq!(resolved.branch, None);
    Ok(())
}

#[test]
fn a_directory_name_that_cannot_name_a_project_asks_for_a_name() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "my app")?;

    let error = resolve(dir.path(), &repository, None).refused_because("a space")?;
    assert_eq!(error.first_id(), Some(ErrorId::LocalNameUnusable));

    let named = resolve(dir.path(), &repository, Some("tool")).required()?;
    assert_eq!(named.identity.display_id(), "local/tool");

    let error = resolve(dir.path(), &repository, Some("not valid")).refused_because("a space")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidProjectId));
    Ok(())
}

#[test]
fn a_path_that_is_not_the_top_of_a_work_tree_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    std::fs::create_dir_all(repository.join("src")).required()?;
    let bare = dir.path().join("bare.git");
    std::fs::create_dir_all(&bare).required()?;
    git_in(&bare, &["init", "--quiet", "--bare"])?;
    let plain = dir.path().join("plain");
    std::fs::create_dir_all(&plain).required()?;
    let file = dir.path().join("file");
    std::fs::write(&file, "not a repository\n").required()?;

    for path in [
        repository.join("src"),
        bare,
        plain,
        file,
        dir.path().join("missing"),
    ] {
        let error = resolve(dir.path(), &path, None).refused_because("not a work tree top")?;
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

    let resolved = resolve(dir.path(), &repository, None).required()?;

    assert_eq!(resolved.branch.as_deref(), Some("main"));
    Ok(())
}

#[test]
fn a_directory_inside_a_work_tree_is_refused_by_naming_the_top() -> Checked {
    // 呼び出し元の`GIT_DIR`は引き継がないが、上のdirectoryのrepositoryは探す。途中を
    // 指されたら、最上位を示して断る。
    let dir = tempfile::tempdir().required()?;
    let repository = work_tree(dir.path(), "app")?;
    let inside = repository.join("src");
    std::fs::create_dir_all(&inside).required()?;

    let error = resolve(dir.path(), &inside, None).refused_because("not the top")?;

    let reason = crate::design::Fact::reason(crate::msg!(
        "cause-working-tree-elsewhere",
        expected = crate::paths::display(&inside),
        observed = crate::paths::display(&repository)
    ));
    assert!(
        error.diagnostics()[0].facts.contains(&reason),
        "{:?}",
        error.diagnostics()[0].facts
    );
    Ok(())
}
