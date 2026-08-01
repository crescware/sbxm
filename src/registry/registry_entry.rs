use std::path::{Path, PathBuf};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths::{self};
use crate::project::{CanonicalProjectId, SandboxName};
use crate::repository::RepositoryIdentity;

/// registryの1 entry。
///
/// 登録意図そのものであり、project rootやmetadataがまだ無い状態も有効なentryとする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    project_root: PathBuf,
    repository: RepositoryIdentity,
}

impl RegistryEntry {
    /// validation済みのproject rootとrepository identityからentryを作る。
    pub fn new(project_root: &Path, repository: RepositoryIdentity) -> Result<RegistryEntry> {
        Ok(RegistryEntry {
            project_root: require_absolute_root(project_root)?,
            repository,
        })
    }

    /// 突き合わせの正本。
    pub fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }

    /// 保存済みの絶対project root。
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn repository(&self) -> &RepositoryIdentity {
        &self.repository
    }

    /// canonical project `IDから決定的に導出したSandbox名`。
    pub fn sandbox_name(&self) -> SandboxName {
        SandboxName::derive(self.canonical_id())
    }
}

/// project rootとして保存してよいpathか。
fn require_absolute_root(declared: &Path) -> Result<PathBuf> {
    let rejected = |reason: Msg| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::RegistryInvalidValue,
                msg!("error-registry-invalid-value"),
            )
            .fact(Fact::field("project_root"))
            .fact(Fact::value(&paths::display(declared)))
            .fact(Fact::reason(reason)),
        ))
    };
    if !declared.is_absolute() {
        return rejected(msg!("cause-not-absolute"));
    }
    let standardized = paths::lexically_standardize(declared);
    if standardized != declared {
        return rejected(msg!(
            "cause-relative-component",
            standardized = paths::display(&standardized)
        ));
    }
    Ok(standardized)
}
