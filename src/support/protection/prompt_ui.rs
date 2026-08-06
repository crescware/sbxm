use crate::design::PromptUi;
use crate::diagnostics::{Msg, Result};

use super::ConfirmPrompt;

impl ConfirmPrompt for PromptUi {
    /// 貼り付けによる前後の空白は、入力そのものの一部として扱わない。
    fn read_sandbox_name(&mut self, heading: &Msg) -> Result<String> {
        Ok(self.exact(heading)?.trim().to_string())
    }
}

#[cfg(test)]
#[path = "prompt_ui_test.rs"]
mod prompt_ui_test;
