use crate::boundary::host::protocol::SandboxState;
use crate::support::Observed;
use crate::support::files::PlacedFile;

use super::{ProvisioningState, WorktreeRow};

/// 初回構築に関係する成果物を、一度のworkflowで観測した結果。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Observation {
    pub state: ProvisioningState,
    pub current_generation: String,
    pub stored_generation: String,
    pub target_generation: String,
    pub stored_image_present: bool,
    pub stored_image_matches: bool,
    pub current_image_present: bool,
    pub current_image_matches: bool,
    pub stored_template_present: bool,
    pub current_template_present: bool,
    pub sandbox_present: bool,
    pub workspace_present: bool,
    pub sandbox_state: Option<SandboxState>,
    pub files_complete: bool,
    pub files: Vec<PlacedFile>,
    pub identity_complete: bool,
    pub tools_complete: bool,
    pub credentials_isolated: bool,
    pub secret_present: bool,
    pub credential_helper: Observed,
    pub repository_complete: bool,
    pub worktrees_complete: bool,
    pub worktrees: Vec<WorktreeRow>,
}

impl Observation {
    pub(crate) fn new(
        state: ProvisioningState,
        current_generation: String,
        stored_generation: String,
        target_generation: String,
    ) -> Self {
        Self {
            state,
            current_generation,
            stored_generation,
            target_generation,
            stored_image_present: false,
            stored_image_matches: false,
            current_image_present: false,
            current_image_matches: false,
            stored_template_present: false,
            current_template_present: false,
            sandbox_present: false,
            workspace_present: false,
            sandbox_state: None,
            files_complete: false,
            files: Vec::new(),
            identity_complete: false,
            tools_complete: false,
            credentials_isolated: false,
            secret_present: false,
            credential_helper: Observed::Missing,
            repository_complete: false,
            worktrees_complete: false,
            worktrees: Vec::new(),
        }
    }

    pub(crate) fn has_partial_artifact(&self) -> bool {
        self.stored_image_present
            || self.current_image_present
            || self.stored_template_present
            || self.current_template_present
            || self.sandbox_present
            || self.workspace_present
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.sandbox_present
            && self.workspace_present
            && self.files_complete
            && self.identity_complete
            && self.tools_complete
            && self.credentials_isolated
            && self.secret_present
            && self.credential_helper.is_matching()
            && self.repository_complete
            && self.worktrees_complete
    }
}
