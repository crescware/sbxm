use std::collections::BTreeMap;

use crate::diagnostics::Result;
use crate::paths;

use super::{HOST_TOOLS_VERSION, RawHostTools, RawTool, RecordedTool};

/// 記録をYAMLへ描画する。memory上の完全な記録を毎回描き直す。
pub(super) fn render(records: &BTreeMap<String, RecordedTool>) -> Result<String> {
    let raw = RawHostTools {
        version: Some(HOST_TOOLS_VERSION),
        tools: records
            .iter()
            .map(|(program, record)| {
                let identity = &record.identity;
                (
                    program.clone(),
                    RawTool {
                        path: Some(paths::display(&identity.path)),
                        device: Some(identity.device),
                        inode: Some(identity.inode),
                        size: Some(identity.size),
                        mtime_secs: Some(identity.mtime_secs),
                        mtime_nanos: Some(identity.mtime_nanos),
                        ctime_secs: Some(identity.ctime_secs),
                        ctime_nanos: Some(identity.ctime_nanos),
                        observed: Some(record.observed.to_string()),
                        digest: Some(record.digest.clone()),
                    },
                )
            })
            .collect(),
    };
    crate::config::serialized(&raw, "host-tools.yaml")
}
