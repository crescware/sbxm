use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RawGitIdentity {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
}
