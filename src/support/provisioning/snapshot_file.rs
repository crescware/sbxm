use crate::config::FileDeclaration;

/// intentが固定した、宣言file 1件の不変snapshot。
#[derive(Debug)]
pub(crate) struct SnapshotFile {
    /// `source`がsnapshot pathを指す、配置用の宣言。
    pub declaration: FileDeclaration,
    pub sha256: String,
    /// 利用者が宣言した本来のsource path。intentの記録にだけ使う。
    pub original_source: String,
}
