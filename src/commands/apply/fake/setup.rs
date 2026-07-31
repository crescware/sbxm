use crate::testing::outcome::{Checked, Required};

use std::path::PathBuf;

use crate::config::{ConfigLocation, FileDeclaration, GlobalConfig};
use crate::i18n::Locale;
use crate::paths::ProjectParent;
use crate::project::SandboxName;

use super::canonical;

pub fn setup(
    files: Vec<FileDeclaration>,
) -> Checked<(
    tempfile::TempDir,
    ConfigLocation,
    ProjectParent,
    GlobalConfig,
    PathBuf,
)> {
    let dir = tempfile::tempdir().required()?;
    let base = dir.path().join("Projects");
    std::fs::create_dir_all(&base).required()?;
    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files,
    };
    let parent = ProjectParent::at(&base).required()?;
    let workspace_root = dir.path().join("workspaces");
    std::fs::create_dir_all(workspace_root.join(SandboxName::derive(&canonical()?).as_str()))
        .required()?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    Ok((dir, location, parent, config, workspace_root))
}
