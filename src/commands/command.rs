use crate::project::ProjectId;

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Add(crate::commands::add::Args),
    Apply(crate::commands::apply::Args),
    Prepare(Option<ProjectId>),
    Rebuild(Option<ProjectId>),
    Open(crate::commands::open::Args),
    Stop(Vec<ProjectId>),
    Ls,
    Status(crate::commands::status::Scope),
    Destroy(crate::commands::destroy::Args),
}
