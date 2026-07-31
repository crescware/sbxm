use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

/// 登録済みの1案件。
#[derive(Debug, Clone)]
pub struct Registered {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub sandbox: SandboxName,
}
