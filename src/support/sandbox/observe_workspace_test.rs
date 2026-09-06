use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::diagnostics::ErrorId;
use crate::project::SandboxName;
use crate::testing::outcome::{Checked, Refused, Required};

use super::observe_workspace;

#[test]
fn a_workspace_root_that_cannot_be_stat_ed_is_unobservable() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let root = dir.path().join("root");
    fs::create_dir_all(&root).required_because("create the workspace root")?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000))
        .required_because("block traversal into the workspace root")?;

    let name = SandboxName::derive(&crate::testing::repository::canonical()?);
    let error = observe_workspace(&root, &name, true)
        .refused_because("a workspace that cannot be stat-ed is not a verified post-condition");

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .required_because("restore permissions so the temp directory can be cleaned up")?;
    let error = error?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    Ok(())
}
