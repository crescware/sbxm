use crate::project::ProjectId;

use super::Topic;

/// `guide`の引数。省略値は対話実行でだけ選択する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub topic: Option<Topic>,
    pub project: Option<ProjectId>,
}
