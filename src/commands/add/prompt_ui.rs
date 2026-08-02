use crate::design::PromptUi;
use crate::diagnostics::Result;
use crate::msg;

use super::IdentityPrompt;

impl IdentityPrompt for PromptUi {
    fn git_user_name(&mut self, candidate: &str) -> Result<String> {
        self.input(&msg!("prompt-git-user-name"), candidate)
    }

    fn git_user_email(&mut self, candidate: &str) -> Result<String> {
        self.input(&msg!("prompt-git-user-email"), candidate)
    }
}

#[cfg(test)]
#[path = "prompt_ui_test.rs"]
mod prompt_ui_test;
