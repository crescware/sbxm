use crate::project::ProjectId;

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// parserが組み立てたhelp本文をstdoutへ提示する。
    Help(String),
    /// parserが組み立てたversion行をstdoutへ提示する。
    Version(String),
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
