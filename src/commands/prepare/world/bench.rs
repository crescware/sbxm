//! `add`から`prepare`までを通しで動かす台。

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::commands::add::run::AddRequest;
use crate::config::{ConfigLocation, GitIdentity, GlobalConfig};
use crate::error::Result;
use crate::i18n::Locale;
use crate::metadata::{self, ProjectMetadata};
use crate::paths::{self, AbsoluteBasePath, PRIVATE_DIR_MODE, ProjectPaths};
use crate::project::ProjectId;

use super::super::run::{PrepareOutput, run};
use super::World;
use crate::ui::SilentProgress;

/// 宣言file 1件を持つ、実行時と同じ形の入力一式。
pub struct Bench {
    pub _base: tempfile::TempDir,
    pub _home: tempfile::TempDir,
    pub workspace_root: tempfile::TempDir,
    pub location: ConfigLocation,
    pub config: GlobalConfig,
}

pub fn bench() -> Bench {
    let base = tempfile::tempdir().expect("temporary base path");
    let home = tempfile::tempdir().expect("temporary home");
    let workspace_root = tempfile::tempdir().expect("temporary workspace root");
    fs::set_permissions(
        workspace_root.path(),
        fs::Permissions::from_mode(PRIVATE_DIR_MODE),
    )
    .expect("the workspace root belongs to the current user only");

    let source = home.path().join("declared.yaml");
    fs::write(&source, b"declared = true\n").expect("write the declared file");

    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(base.path()).expect("valid base path"),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: vec![crate::config::FileDeclaration {
            source: crate::config::HostFileSource::new(&paths::display(&source))
                .expect("valid source"),
            destination: crate::config::SandboxHomeRelativePath::new(
                ".config/example/settings.yaml",
            )
            .expect("valid destination"),
        }],
    };
    Bench {
        location: ConfigLocation::from_home(home.path().to_path_buf()),
        _base: base,
        _home: home,
        workspace_root,
        config,
    }
}

impl Bench {
    /// `add`で登録してから`prepare`で構築する。工程は通しで判定する。
    pub fn build(&self, world: &World, request: &AddRequest) -> Result<PrepareOutput> {
        crate::commands::add::run::run(
            &self.location,
            &self.config,
            request,
            world,
            &mut SilentProgress,
        )?;
        let project = ProjectId::parse(&request.repository.display_id())
            .expect("the registered repository names a project");
        run(
            &self.location,
            &self.config,
            &project,
            world,
            self.workspace_root.path(),
            &mut SilentProgress,
        )
    }

    pub fn stored(&self, project: &str) -> ProjectMetadata {
        let canonical = ProjectId::parse(project).unwrap().canonical();
        let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
        metadata::load(&paths)
            .expect("read the metadata")
            .expect("present")
    }
}
