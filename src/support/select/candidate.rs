use crate::diagnostics::Result;
use crate::metadata::{self, ProjectMetadata};
use std::time::Duration;

use crate::paths::{self, LOCK_TIMEOUT, PathScope, ProjectPaths};
use crate::repository::RepositoryIdentity;

use super::{Locked, incomplete_registration, inconsistent_registration};

/// 選択された1案件。runtime状態は持たない。
///
/// 表示に必要な情報はregistry entryで揃う。`open`のpromptが範囲を示すために読むmetadataは
/// 表示だけに使い、判定はlockを取ってから読み直す。
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

    /// registry entryが指すmetadataを読み直す。
    ///
    /// lockを取らずに読んだmetadataは読んだ直後に古くなり得るため、表示や早い拒否にだけ使い、
    /// preconditionの判定にはlockを取ってから読み直したものを使う。
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
        self.lock_within(LOCK_TIMEOUT)
    }

    /// project lockを`wait`だけ待って取り、lock後の内容で読み直す。
    pub fn lock_within(self, wait: Duration) -> Result<Locked> {
        self.verify_root()?;
        let lock = self.paths.acquire_lock_within(wait)?;
        let metadata = self.reload()?;
        Ok(Locked {
            paths: self.paths,
            metadata,
            _lock: lock,
        })
    }
}
