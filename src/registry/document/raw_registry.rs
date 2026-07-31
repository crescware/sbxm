use serde::{Deserialize, Serialize};

use super::RawEntry;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawRegistry {
    pub version: Option<i64>,
    #[serde(default)]
    pub projects: Vec<RawEntry>,
}
