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

    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximum_index: u32,
    ) -> Result<(usize, u32)> {
        PromptUi::select_open(self, heading, candidates, maximum_index)
    }

    fn select_open_with_maximums(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximum_index: u32,
        maximums: &mut dyn FnMut(usize) -> Option<u32>,
    ) -> Result<(usize, u32)> {
        PromptUi::select_open_with_maximums(self, heading, candidates, maximum_index, maximums)
    }
}

#[cfg(test)]
#[path = "prompt_ui_test.rs"]
mod prompt_ui_test;
