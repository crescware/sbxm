use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::boundary::host::ExecutableIdentity;
use crate::boundary::host::protocol::CliVersion;

use super::{HOST_TOOLS_VERSION, RawHostTools, RawTool, RecordedTool};

/// 記録のtextを読む。解釈できない記録は`None`とし、推測で埋めない。
///
/// 未知のkey、欠けたfield、読めないversion表記、未知の文書versionはすべて解釈できない
/// 記録である。記録は確認を省略する根拠であり、一部でも疑わしければ全体を使わない。
pub(super) fn parse(text: &str) -> Option<BTreeMap<String, RecordedTool>> {
    let raw: RawHostTools = yaml_serde::from_str::<Option<RawHostTools>>(text).ok()??;
    if raw.version? != HOST_TOOLS_VERSION {
        return None;
    }
    let mut records = BTreeMap::new();
    for (program, tool) in raw.tools {
        records.insert(program, recorded(tool)?);
    }
    Some(records)
}

fn recorded(tool: RawTool) -> Option<RecordedTool> {
    Some(RecordedTool {
        identity: ExecutableIdentity {
            path: PathBuf::from(tool.path?),
            device: tool.device?,
            inode: tool.inode?,
            size: tool.size?,
            mtime_secs: tool.mtime_secs?,
            mtime_nanos: tool.mtime_nanos?,
            ctime_secs: tool.ctime_secs?,
            ctime_nanos: tool.ctime_nanos?,
        },
        observed: CliVersion::parse(&tool.observed?)?,
        digest: tool.digest?,
    })
}
