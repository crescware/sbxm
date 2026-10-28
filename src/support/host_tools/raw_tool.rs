use serde::{Deserialize, Serialize};

/// 1つの道具の記録の生表現。
///
/// 未知のkeyは拒む。記録は確認を省略する根拠であり、知らないものが混ざった記録を
/// 推測で読まない。
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTool {
    pub(super) path: Option<String>,
    pub(super) device: Option<u64>,
    pub(super) inode: Option<u64>,
    pub(super) size: Option<u64>,
    pub(super) mtime_secs: Option<i64>,
    pub(super) mtime_nanos: Option<i64>,
    pub(super) ctime_secs: Option<i64>,
    pub(super) ctime_nanos: Option<i64>,
    pub(super) observed: Option<String>,
    pub(super) digest: Option<String>,
}
