use crate::project::SandboxName;

use crate::support::inventory::ProjectState;

use super::{StopOutcome, StopResult};

/// 停止対象1件。
pub(super) struct Target {
    pub(super) display_id: String,
    pub(super) sandbox: SandboxName,
    pub(super) state: ProjectState,
}

impl Target {
    pub(crate) fn outcome(&self, result: StopResult) -> StopOutcome {
        StopOutcome {
            project: self.display_id.clone(),
            sandbox: self.sandbox.as_str().to_string(),
            result,
        }
    }
}
