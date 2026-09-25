use std::path::PathBuf;

use crate::config::FileDeclaration;

/// `files add`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub declaration: FileDeclaration,
    /// 宣言を持つconfig。
    pub path: PathBuf,
    /// 同じ宣言が既にあり、configを変えなかった。
    pub already: bool,
    /// 認証情報を持つfileによく使われる名前だった。
    pub credential_like: bool,
}
