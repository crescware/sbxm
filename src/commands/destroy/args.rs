use crate::project::ProjectId;

/// `destroy`の対象と mode。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// force modeはTTYかどうかにかかわらずproject引数の完全指定を必須とする。
    pub project: Option<ProjectId>,
    pub force: bool,
}
