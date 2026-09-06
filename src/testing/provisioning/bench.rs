//! `add`から初回構築までを通しで動かす台。

use crate::testing::outcome::{Checked, Required};

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::commands::add::AddRequest;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::diagnostics::Result;
use crate::i18n::Locale;
use crate::metadata::{self, ProjectMetadata};
use crate::paths::{self, PRIVATE_DIR_MODE, ProjectParent, ProjectPaths};
use crate::project::ProjectId;

use crate::support::provisioning::{ProvisioningOutput, ensure_initial};
use crate::support::select;

use super::World;
use crate::design::{ProgressSink, SilentProgress};

/// 宣言file 1件を持つ、実行時と同じ形の入力一式。
pub struct Bench {
    pub parent: ProjectParent,
    pub _base: tempfile::TempDir,
    pub _home: tempfile::TempDir,
    pub workspace_root: tempfile::TempDir,
    pub location: ConfigLocation,
    pub config: GlobalConfig,
}

impl Bench {
    pub fn new() -> Checked<Bench> {
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

    /// `add`で登録するだけで、Sandboxもimageも作らない。
    pub fn register(&self, world: &World, request: &AddRequest) -> Result<ProjectId> {
        crate::commands::add::run::run(
            &self.location,
            &self.parent,
            request,
            &crate::testing::metadata::git_identity(),
            world,
            &mut SilentProgress,
        )?;
        ProjectId::parse(&request.repository.display_id())
    }

    /// 登録済み案件の初回構築を進める。
    ///
    /// 入口commandではなく共有境界を直接使い、どのcommandの寿命にも縛られない。対象の
    /// 解決とlockは入口commandと同じ順序で通す。
    pub fn ensure(
        &self,
        world: &World,
        project: &ProjectId,
        progress: &mut dyn ProgressSink,
    ) -> Result<ProvisioningOutput> {
        self.ensure_with(world, project, &self.config, progress)
    }

    /// 別の設定で初回構築を進める。構築後に宣言fileを足した場合の挙動を見るために使う。
    pub fn ensure_with(
        &self,
        world: &World,
        project: &ProjectId,
        config: &GlobalConfig,
        progress: &mut dyn ProgressSink,
    ) -> Result<ProvisioningOutput> {
        let mut locked = select::find(&self.location, project)?.lock()?;
        ensure_initial(
            &mut locked,
            config,
            world,
            self.workspace_root.path(),
            progress,
        )
    }

    /// `add`で登録してから初回構築を通す。工程は通しで判定する。
    pub fn build(&self, world: &World, request: &AddRequest) -> Result<ProvisioningOutput> {
        let project = self.register(world, request)?;
        self.ensure(world, &project, &mut SilentProgress)
    }

    pub fn stored(&self, project: &str) -> Checked<ProjectMetadata> {
        let canonical = ProjectId::parse(project).required()?.canonical();
        let paths = ProjectPaths::derive(&self.parent, &canonical);
        metadata::load(&paths)
            .required_because("read the metadata")?
            .required_because("present")
    }
}
