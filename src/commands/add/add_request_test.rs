use std::path::Path;

use crate::boundary::host::RealHost;
use crate::paths::ProjectParent;
use crate::testing::outcome::{Checked, Required};
use crate::testing::repository::git_in;

use super::super::{AddTarget, Args};
use super::AddRequest;

fn local_args(path: &Path, detach: Option<&str>) -> Args {
    Args {
        target: AddTarget::Local {
            path: path.to_path_buf(),
            name: None,
        },
        worktrees: None,
        detach: detach.map(str::to_owned),
        git_identity: None,
    }
}

fn repository(parent: &Path) -> Checked<std::path::PathBuf> {
    let path = parent.join("app");
    std::fs::create_dir_all(&path).required()?;
    git_in(&path, &["init", "--quiet"])?;
    git_in(
        &path,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;
    Ok(path)
}

#[test]
fn a_host_repository_starts_from_the_branch_it_is_on() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = repository(dir.path())?;
    let parent = ProjectParent::at(dir.path()).required()?;

    let request = AddRequest::resolve(&local_args(&path.join(".git"), None), &parent, &RealHost)
        .required()?;
    assert_eq!(request.start_branch.as_deref(), Some("main"));
    assert_eq!(request.repository.display_id(), "local/app");
    assert!(!request.parent_inside_repository);

    // 起点を明示すれば、今いるbranchは使わない。
    let request = AddRequest::resolve(
        &local_args(&path.join(".git"), Some("develop")),
        &parent,
        &RealHost,
    )
    .required()?;
    assert_eq!(request.start_branch, None);
    assert_eq!(request.detach.as_deref(), Some("develop"));
    Ok(())
}

#[test]
fn a_detached_host_repository_carries_no_start_branch() -> Checked {
    // 起点が無いことを断るのは、新しく記録するときである。登録済みの案件は、保存済みの
    // 起点で続けられる。
    let dir = tempfile::tempdir().required()?;
    let path = repository(dir.path())?;
    git_in(&path, &["checkout", "--quiet", "--detach"])?;
    let parent = ProjectParent::at(dir.path()).required()?;

    let request = AddRequest::resolve(&local_args(&path.join(".git"), None), &parent, &RealHost)
        .required()?;
    assert_eq!(request.start_branch, None);
    Ok(())
}

#[test]
fn a_host_repository_carries_whether_the_project_would_sit_inside_it() -> Checked {
    // ここでは断らない。断るのは、案件directoryを新しく作る登録である。
    let dir = tempfile::tempdir().required()?;
    let path = repository(dir.path())?;
    let inside = ProjectParent::at(&path).required()?;

    let request = AddRequest::resolve(&local_args(&path.join(".git"), None), &inside, &RealHost)
        .required()?;
    assert!(request.parent_inside_repository);
    Ok(())
}
