use std::path::PathBuf;

/// Sandboxから受け取ったbundle。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedBundle {
    pub path: PathBuf,
    /// 受け取った時刻を表す名前。退避するrefの名前にも使い、bundleと対応させる。
    pub label: String,
}
