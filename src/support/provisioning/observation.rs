use crate::boundary::host::protocol::SandboxState;
use crate::diagnostics::{Error, Result};
use crate::support::Observed;
use crate::support::files::PlacedFile;

use super::{ProvisioningState, WorktreeRow};

/// 初回構築に関係する成果物を、一度のworkflowで観測した結果。
///
/// artifactごとの結果は`Observed`で持ち、「見て無かった」と「見ていない」を混ぜない。
/// 観測の途中で見つけた安全でない事実は`blocking`へ残す。変更を伴うcommandは
/// `require_safe`で拒否し、read-onlyな診断は観測を最後まで並べられる。
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
    pub sandbox: Observed,
    pub sandbox_state: Option<SandboxState>,
    pub workspace: Observed,
    pub files_placed: Observed,
    pub files: Vec<PlacedFile>,
    pub identity: Observed,
    pub tools: Observed,
    pub credentials: Observed,
    pub secret: Observed,
    pub credential_helper: Observed,
    pub repository: Observed,
    pub worktrees_present: Observed,
    pub worktrees: Vec<WorktreeRow>,
    blocking: Vec<Error>,
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
            sandbox: Observed::Missing,
            sandbox_state: None,
            workspace: Observed::Missing,
            files_placed: Observed::Missing,
            files: Vec::new(),
            identity: Observed::Missing,
            tools: Observed::Missing,
            credentials: Observed::Missing,
            secret: Observed::Missing,
            credential_helper: Observed::Missing,
            repository: Observed::Missing,
            worktrees_present: Observed::Missing,
            worktrees: Vec::new(),
            blocking: Vec::new(),
        }
    }

    /// 観測の間に集めた「安全と確認できなかった事実」を記録する。
    pub(crate) fn block_all(&mut self, errors: Vec<Error>) {
        self.blocking = errors;
    }

    /// 変更を始める前に、安全と確認できなかった事実があれば拒否する。
    ///
    /// 観測そのものは最後まで進めるため、mutationを持つcommandはこれを必ず通る。
    pub(crate) fn require_safe(&self) -> Result<()> {
        match self.blocking.first() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// 目標構成の判定に使うartifact一式。
    fn required(&self) -> [&Observed; 10] {
        [
            &self.sandbox,
            &self.workspace,
            &self.files_placed,
            &self.identity,
            &self.tools,
            &self.credentials,
            &self.secret,
            &self.credential_helper,
            &self.repository,
            &self.worktrees_present,
        ]
    }

    pub(crate) fn has_partial_artifact(&self) -> bool {
        self.stored_image_present
            || self.current_image_present
            || self.stored_template_present
            || self.current_template_present
            || !self.sandbox.is_missing()
            || !self.workspace.is_missing()
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.required()
            .iter()
            .all(|observed| observed.is_matching())
    }

    /// Sandboxを起動しないため、内部のartifactを1件も観測しなかった。
    pub(crate) fn interior_is_unobservable(&self) -> bool {
        [
            &self.files_placed,
            &self.identity,
            &self.tools,
            &self.credentials,
            &self.secret,
            &self.credential_helper,
            &self.repository,
            &self.worktrees_present,
        ]
        .into_iter()
        .all(|observed| matches!(observed, Observed::Unobservable { .. }))
    }

    /// 欠落または食い違いを実際に観測した。観測不能はここへ含めない。
    pub(crate) fn has_definite_gap(&self) -> bool {
        self.required()
            .iter()
            .any(|observed| observed.is_missing() || matches!(observed, Observed::Mismatch { .. }))
    }

    /// 観測結果を共有の状態へ分類する唯一の規則。
    ///
    /// 変更を伴うcommandもread-onlyな診断も、同じ事実へ別の規則を当てない。観測
    /// できなかっただけの案件を`Incomplete`にせず、完成の証明がない案件を`Ready`にも
    /// しない。
    pub(crate) fn classify(&self, has_intent: bool) -> ProvisioningState {
        if has_intent {
            return ProvisioningState::Pending;
        }
        if !self.has_partial_artifact() {
            return ProvisioningState::Fresh;
        }
        if self.is_complete() {
            return ProvisioningState::Ready;
        }
        if self.has_definite_gap() {
            return ProvisioningState::Incomplete;
        }
        ProvisioningState::Unobservable
    }
}

#[cfg(test)]
#[path = "observation_test.rs"]
mod observation_test;
