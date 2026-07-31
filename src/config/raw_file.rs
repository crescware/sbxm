use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawFile {
    pub(super) source: Option<String>,
    pub(super) destination: Option<String>,
}
