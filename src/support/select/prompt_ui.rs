use crate::design::PromptUi;
use crate::diagnostics::{Msg, Result};

use super::ProjectPrompt;

impl ProjectPrompt for PromptUi {
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize> {
        PromptUi::select_one(self, heading, candidates)
    }

    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>> {
        PromptUi::select_many(self, heading, candidates)
    }
}
