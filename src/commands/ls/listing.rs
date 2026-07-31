use super::{ProjectRow, UnmanagedRow};

/// `ls`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub projects: Vec<ProjectRow>,
    pub unmanaged: Vec<UnmanagedRow>,
    /// 全案件が登録済みで、成果物がregistryと一致しているか。
    pub settled: bool,
}
