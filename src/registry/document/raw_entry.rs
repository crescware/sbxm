use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawEntry {
    pub canonical_id: Option<String>,
    pub project_root: Option<String>,
    pub provider: Option<String>,
    pub clone_transport: Option<String>,
    pub clone_url: Option<String>,
}
