use crate::diagnostics::Result;
use crate::metadata::{self, ProjectMetadata};
use crate::paths::{self, PathScope, ProjectPaths};
use crate::repository::RepositoryIdentity;

use super::{Locked, incomplete_registration, inconsistent_registration};

/// 選択された1案件。runtime状態は持たない。
///
/// 表示に必要な情報はregistry entryだけで揃う。metadataはlockを取ってから読む。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub paths: ProjectPaths,
    pub repository: RepositoryIdentity,
}

impl Candidate {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.repository.display_id()
    }

    /// lock取得後に読み直したmetadata。
    ///
    /// 選択時に読んだmetadataは古くなり得るため、preconditionの判定にはこちらを使う。
    /// registry entryと一致しないmetadataは、どちらかを正しいものとして採用しない。
    pub fn reload(&self) -> Result<ProjectMetadata> {
        self.verify_root()?;
        let Some(metadata) = metadata::load(&self.paths)? else {
            return Err(incomplete_registration(self));
        };
        if !metadata.repository.same_target(&self.repository) {
            return Err(inconsistent_registration(
                &self.paths,
                &metadata,
                &self.repository,
            ));
        }
        Ok(metadata)
    }

    /// registryが指すproject rootを、信用する前に観測する。
    ///
    /// 保存されたabsolute pathであっても、そこにdirectoryがあり、現在の利用者が
    /// 所有していることを確かめてから読み書きする。
    fn verify_root(&self) -> Result<()> {
        paths::require_owned_directory(self.paths.root(), PathScope::ProjectPath)
    }

    /// project lockを取り、lock後の内容で読み直す。
    pub fn lock(self) -> Result<Locked> {
        self.verify_root()?;
        let lock = self.paths.acquire_lock()?;
        let metadata = self.reload()?;
        Ok(Locked {
            paths: self.paths,
            metadata,
            _lock: lock,
        })
    }
}
