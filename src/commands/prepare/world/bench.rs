//! `add`から`prepare`までを通しで動かす台。

use crate::testing::outcome::{Checked, Required};

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::commands::add::run::AddRequest;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::error::Result;
use crate::i18n::Locale;
use crate::metadata::{self, ProjectMetadata};
use crate::paths::{self, PRIVATE_DIR_MODE, ProjectParent, ProjectPaths};
use crate::project::ProjectId;

use super::super::run::{PrepareOutput, run};
use super::World;
use crate::ui::SilentProgress;

/// 宣言file 1件を持つ、実行時と同じ形の入力一式。
pub struct Bench {
    pub parent: ProjectParent,
    pub _base: tempfile::TempDir,
    pub _home: tempfile::TempDir,
    pub workspace_root: tempfile::TempDir,
    pub location: ConfigLocation,
    pub config: GlobalConfig,
}

pub fn bench() -> Checked<Bench> {
    let base = tempfile::tempdir().required_because("temporary base path")?;
    let home = tempfile::tempdir().required_because("temporary home")?;
    let workspace_root = tempfile::tempdir().required_because("temporary workspace root")?;
    fs::set_permissions(
        workspace_root.path(),
        fs::Permissions::from_mode(PRIVATE_DIR_MODE),
    )
    .required_because("the workspace root belongs to the current user only")?;

    let source = home.path().join("declared.yaml");
    fs::write(&source, b"declared = true\n").required_because("write the declared file")?;

    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: vec![crate::config::FileDeclaration {
            source: crate::config::HostFileSource::new(&paths::display(&source))
                .required_because("valid source")?,
            destination: crate::config::SandboxHomeRelativePath::new(
                ".config/example/settings.yaml",
            )
            .required_because("valid destination")?,
        }],
    };
    Ok(Bench {
        location: ConfigLocation::from_home(home.path().to_path_buf()),
        parent: ProjectParent::at(base.path()).required_because("valid parent directory")?,
        _base: base,
        _home: home,
        workspace_root,
        config,
    })
}

impl Bench {
    /// `add`で登録してから`prepare`で構築する。工程は通しで判定する。
    pub fn build(&self, world: &World, request: &AddRequest) -> Result<PrepareOutput> {
        crate::commands::add::run::run(
            &self.location,
            &self.parent,
            request,
            &crate::testing::metadata::git_identity(),
            world,
            &mut SilentProgress,
        )?;
        let project = ProjectId::parse(&request.repository.display_id())?;
        run(
            &self.location,
            &self.config,
            &project,
            world,
            self.workspace_root.path(),
            &mut SilentProgress,
        )
    }

    pub fn stored(&self, project: &str) -> Checked<ProjectMetadata> {
        let canonical = ProjectId::parse(project).required()?.canonical();
        let paths = ProjectPaths::derive(&self.parent, &canonical);
        metadata::load(&paths)
            .required_because("read the metadata")?
            .required_because("present")
    }
}
