use std::path::PathBuf;

use crate::repository::RepositoryIdentity;

/// `add`が登録するrepositoryの指定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddTarget {
    /// GitHub repositoryのclone URLを解釈したもの。
    Clone(RepositoryIdentity),
    /// hostにあるrepositoryのgit directory。実在を確かめて正規化するのは登録の直前である。
    Local {
        path: PathBuf,
        /// 案件の名前。省略すれば`git clone`が作るdirectoryの名前を使う。
        name: Option<String>,
    },
}
