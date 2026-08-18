use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RawInitialProvisioning {
    pub target_dockerfile_sha256: Option<String>,
}
