use crate::diagnostics::{Error, Msg, Result};
use crate::support::select::ProjectPrompt;

/// 選択結果を決め打ちするprompt。
pub struct ScriptedPrompt {
    pub one: usize,
    pub many: Vec<usize>,
    pub canceled: bool,
    pub asked: std::cell::RefCell<Vec<Vec<String>>>,
    /// 訊かれた見出し。commandが何を訊いたかをtestが確かめる。
    pub headings: std::cell::RefCell<Vec<&'static str>>,
}

impl ScriptedPrompt {
    pub fn choosing(one: usize) -> ScriptedPrompt {
        ScriptedPrompt {
            one,
            many: Vec::new(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
            headings: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: many.to_vec(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
            headings: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn canceling() -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: Vec::new(),
            canceled: true,
            asked: std::cell::RefCell::new(Vec::new()),
            headings: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl ProjectPrompt for ScriptedPrompt {
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize> {
        self.record(heading, candidates);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.one)
    }

    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>> {
        self.record(heading, candidates);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.many.clone())
    }
}

impl ScriptedPrompt {
    fn record(&self, heading: &Msg, candidates: &[String]) {
        self.headings.borrow_mut().push(heading.id);
        self.asked.borrow_mut().push(candidates.to_vec());
    }
}
