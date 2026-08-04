use crate::project::ProjectId;

/// `status`のscope。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project(ProjectId),
    Prompt,
}
