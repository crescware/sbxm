//! 選択を決め打ちするprompt。

use crate::error::{Error, Result};
use crate::workflow::select::ProjectPrompt;

/// 選択結果を決め打ちするprompt。
pub struct ScriptedPrompt {
    pub one: usize,
    pub many: Vec<usize>,
    pub canceled: bool,
    pub asked: std::cell::RefCell<Vec<Vec<String>>>,
}

impl ScriptedPrompt {
    pub fn choosing(one: usize) -> ScriptedPrompt {
        ScriptedPrompt {
            one,
            many: Vec::new(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: many.to_vec(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn canceling() -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: Vec::new(),
            canceled: true,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl ProjectPrompt for ScriptedPrompt {
    fn select_one(&mut self, candidates: &[String]) -> Result<usize> {
        self.asked.borrow_mut().push(candidates.to_vec());
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.one)
    }

    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>> {
        self.asked.borrow_mut().push(candidates.to_vec());
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.many.clone())
    }
}
