use crate::project::ProjectId;

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Add(crate::commands::add::Args),
    Apply(crate::commands::apply::Args),
    Prepare(ProjectId),
    Rebuild(ProjectId),
    Open(Option<ProjectId>),
    Stop(Vec<ProjectId>),
    Ls,
    Status(crate::commands::status::Scope),
    Destroy(crate::commands::destroy::Args),
}
