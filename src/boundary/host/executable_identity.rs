use std::path::PathBuf;

/// 実行ファイルの実体を見分ける識別情報。
///
/// pathはsymlinkを解決した実体のものであり、残りはその実体のmetadataである。どれか1つでも
/// 違えば別の実行ファイルとみなす。中身が同じまま識別情報が変わることはあっても、中身が
/// 変わったのに識別情報がすべて同じまま残ることはない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableIdentity {
    /// symlinkを解決した実体のpath。
    pub path: PathBuf,
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: i64,
    pub ctime_secs: i64,
    pub ctime_nanos: i64,
}
