use std::path::Path;

use crate::boundary::host::RealHost;
use crate::diagnostics::ErrorId;
use crate::paths::ProjectParent;
use crate::testing::outcome::{Checked, Refused, Required};
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

    let request = AddRequest::resolve(&local_args(&path, None), &parent, &RealHost).required()?;
    assert_eq!(request.start_branch.as_deref(), Some("main"));
    assert_eq!(request.repository.display_id(), "local/app");

    // 起点を明示すれば、今いるbranchは使わない。
    let request =
        AddRequest::resolve(&local_args(&path, Some("develop")), &parent, &RealHost).required()?;
    assert_eq!(request.start_branch, None);
    assert_eq!(request.detach.as_deref(), Some("develop"));
    Ok(())
}

#[test]
fn a_detached_host_repository_needs_an_explicit_start_branch() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = repository(dir.path())?;
    git_in(&path, &["checkout", "--quiet", "--detach"])?;
    let parent = ProjectParent::at(dir.path()).required()?;

    let error = AddRequest::resolve(&local_args(&path, None), &parent, &RealHost)
        .refused_because("there is no branch to start from")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostRepositoryDetached));

    AddRequest::resolve(&local_args(&path, Some("main")), &parent, &RealHost).required()?;
    Ok(())
}
