use crate::metadata::ProjectMetadata;
use crate::paths::{ExclusiveLock, ProjectPaths};
use crate::project::SandboxName;

/// 登録を終えた案件。
///
/// project lockを保持しているため、この値が生きているあいだ同じ案件へのmutationは
/// 直列化される。
#[derive(Debug)]
pub struct Registration {
    pub paths: ProjectPaths,
    pub sandbox: SandboxName,
    pub metadata: ProjectMetadata,
    pub(super) _lock: ExclusiveLock,
}
