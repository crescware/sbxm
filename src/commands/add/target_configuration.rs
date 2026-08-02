use crate::diagnostics::{ErrorId, Result, fail};
use crate::git;
use crate::metadata::{CreationMode, MAX_WORKTREES, MIN_WORKTREES};
use crate::msg;

use super::AddRequest;

/// optionから決まる目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConfiguration {
    pub mode: CreationMode,
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
}

impl TargetConfiguration {
    /// | 指定 | mode | start_ref | managed数 |
    /// |---|---|---|---:|
    /// | 指定なし | attached | remote default branch | 1 |
    /// | `--detach BRANCH` | detached | BRANCH | 1 |
    /// | `--worktrees N --detach BRANCH` | detached | BRANCH | N |
    pub fn from_request(request: &AddRequest) -> Result<TargetConfiguration> {
        let requested_worktrees = request.worktrees.unwrap_or(MIN_WORKTREES);
        if !(MIN_WORKTREES..=MAX_WORKTREES).contains(&requested_worktrees) {
            return fail(
                ErrorId::WorktreesOutOfRange,
                msg!(
                    "error-worktrees-out-of-range",
                    value = requested_worktrees,
                    minimum = MIN_WORKTREES,
                    maximum = MAX_WORKTREES
                ),
            );
        }

        if let Some(branch) = &request.detach {
            git::validate_branch_name(branch)?;
            Ok(TargetConfiguration {
                mode: CreationMode::Detached,
                start_ref: Some(branch.clone()),
                requested_worktrees,
            })
        } else {
            if requested_worktrees > 1 {
                return fail(
                    ErrorId::WorktreesRequireDetach,
                    msg!("error-worktrees-require-detach"),
                );
            }
            Ok(TargetConfiguration {
                // attached modeのstart refはremote default branchを解決してから確定する。
                mode: CreationMode::Attached,
                start_ref: None,
                requested_worktrees,
            })
        }
    }
}

#[cfg(test)]
#[path = "target_configuration_test.rs"]
mod target_configuration_test;
