//! Project metadata `<project-root>/.sbxm/project.yaml`。
//!
//! metadataは進捗cacheではなく、利用者が要求した目標構成である。sbxmは成果物の
//! 作成元を追跡しないため、validation規則を満たすmetadataは、誰が書いたかを問わず
//! 同じものとして扱う。

mod document;
mod parse;
mod render;

pub use parse::parse;
pub use render::render;

use std::fs;
use std::path::Path;

use crate::error::{DocumentVersion, ErrorId, Result, fail};
use crate::msg;
use crate::paths::{self, PRIVATE_FILE_MODE, ProjectPaths, atomic_create, atomic_replace};
use crate::project::{CanonicalProjectId, SandboxName};
use crate::repository::RepositoryIdentity;

/// このbuildが読み書きするmetadataのversion。
pub const METADATA_VERSION: u32 = 1;
/// metadataのversionの読み方。
const DOCUMENT: DocumentVersion = DocumentVersion {
    supported: METADATA_VERSION,
    unknown: ErrorId::MetadataUnknownVersion,
    unknown_message: "error-metadata-unknown-version",
};
/// managed worktreeの下限と上限。CLIのoption validationと同じ範囲を使う。
pub const MIN_WORKTREES: u32 = 1;
pub const MAX_WORKTREES: u32 = 32;
/// 全managed worktreeの作り方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationMode {
    /// remote default branchをtrackingするlocal branchを1つ作る。
    Attached,
    /// 全managed worktreeを同じ`origin/<BRANCH>` commitから作る。
    Detached,
}
impl CreationMode {
    /// 翻訳しない安定した表記。metadataと利用者向けtableの両方で使う。
    pub fn as_str(self) -> &'static str {
        match self {
            CreationMode::Attached => "attached",
            CreationMode::Detached => "detached",
        }
    }

    fn parse(value: &str) -> Option<CreationMode> {
        match value {
            "attached" => Some(CreationMode::Attached),
            "detached" => Some(CreationMode::Detached),
            _ => None,
        }
    }
}
impl std::fmt::Display for CreationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
/// 利用者が要求した目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisioning {
    pub mode: CreationMode,
    /// 起点branch。attached modeではremote default branchを解決するまで未確定を許す。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    /// 初回構築中は採用世代、構築完了後は適用済みのDockerfile hash。
    pub dockerfile_sha256: String,
}
/// `rebuild`のSandbox切替中だけ存在する適用予定世代。
///
/// `rebuild`が新世代の成果物を揃えた時点で記録し、切替完了で消す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildIntent {
    pub target_dockerfile_sha256: String,
    pub previous_dockerfile_sha256: String,
}
/// `Sandbox内で使用するGit` identity。
///
/// 新規登録時にhostの`git config --global`から取得し、以後は保存値だけを使う。
/// host設定が後から変わっても、登録済み案件のidentityを暗黙変更しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIdentity {
    pub user_name: String,
    pub user_email: String,
}

/// Git identityの値として使えるか。
pub fn validate_git_identity_value(value: &str) -> std::result::Result<(), &'static str> {
    if value.trim().is_empty() {
        return Err("the value is empty");
    }
    if value.contains('\n') || value.contains('\r') {
        return Err("the value contains a line break");
    }
    Ok(())
}

/// 1案件のmetadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetadata {
    /// 登録対象の不変なrepository identity。
    ///
    /// clone URL文字列から実行時にtransportを推測し直さないよう、解釈済みの構造で持つ。
    pub repository: RepositoryIdentity,
    pub provisioning: Provisioning,
    /// `Sandbox内で使うGit` identity。登録時のhost設定のsnapshotである。
    pub git_identity: GitIdentity,
    pub rebuild: Option<RebuildIntent>,
}
impl ProjectMetadata {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.repository.display_id()
    }

    /// 突き合わせの正本となるcanonical project ID。
    pub fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }

    /// canonical project `IDから決定的に導出したSandbox名`。
    pub fn sandbox_name(&self) -> SandboxName {
        SandboxName::derive(self.canonical_id())
    }
}
/// metadataをread-onlyで読む。存在しなければ`None`。
pub fn load(paths: &ProjectPaths) -> Result<Option<ProjectMetadata>> {
    let path = paths.metadata_file();
    match read_optional(&path)? {
        Some(text) => Ok(Some(parse(&text, &path)?)),
        None => Ok(None),
    }
}
/// metadataを新規作成する。既存fileは上書きしない。
pub fn create(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_create(
        &paths.metadata_file(),
        &render(metadata)?,
        PRIVATE_FILE_MODE,
    )
}
/// 既存metadataをatomicに置き換える。
pub fn update(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_replace(
        &paths.metadata_file(),
        &render(metadata)?,
        PRIVATE_FILE_MODE,
    )
}
/// symlinkを追跡せずにmetadataを読む。
fn read_optional(path: &Path) -> Result<Option<String>> {
    if paths::is_symlink(path) {
        // symlinkの先は案件directory外にあり得るため、追跡せず不在として扱わない。
        return fail(
            ErrorId::MetadataUnreadable,
            msg!(
                "error-metadata-unreadable",
                path = paths::display(path),
                detail = "the metadata path is a symbolic link"
            ),
        );
    }
    // 通常fileであることを確かめてから開く。FIFOのような特殊fileを開いて待たない。
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return fail(
                ErrorId::MetadataUnreadable,
                msg!(
                    "error-metadata-unreadable",
                    path = paths::display(path),
                    detail = "the metadata path is not a regular file"
                ),
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return fail(
                ErrorId::MetadataUnreadable,
                msg!(
                    "error-metadata-unreadable",
                    path = paths::display(path),
                    detail = error
                ),
            );
        }
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => fail(
            ErrorId::MetadataUnreadable,
            msg!(
                "error-metadata-unreadable",
                path = paths::display(path),
                detail = error
            ),
        ),
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
