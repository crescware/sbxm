use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::RawTool;

/// `host-tools.yaml`の生表現。
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawHostTools {
    pub(super) version: Option<i64>,
    /// program名ごとの記録。
    #[serde(default)]
    pub(super) tools: BTreeMap<String, RawTool>,
}
