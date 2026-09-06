use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RawInitialProvisioningFile {
    pub source: Option<String>,
    pub destination: Option<String>,
    pub sha256: Option<String>,
}
