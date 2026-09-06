use serde::{Deserialize, Serialize};

use super::RawInitialProvisioningFile;

#[derive(Debug, Deserialize, Serialize)]
pub struct RawInitialProvisioning {
    pub target_dockerfile_sha256: Option<String>,
    pub files: Option<Vec<RawInitialProvisioningFile>>,
}
