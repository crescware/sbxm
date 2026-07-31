use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RawRebuild {
    pub target_dockerfile_sha256: Option<String>,
    pub previous_dockerfile_sha256: Option<String>,
}
