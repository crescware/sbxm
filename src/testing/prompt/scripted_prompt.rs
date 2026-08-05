use crate::diagnostics::{Error, Msg, Result};
use crate::support::select::ProjectPrompt;

/// 選択結果を決め打ちするprompt。
pub struct ScriptedPrompt {
    pub one: usize,
    pub many: Vec<usize>,
    pub index: u32,
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
            index: 0,
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
            headings: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 案件は先頭から選び、worktree indexは指定値で確定する。
    pub fn choosing_worktree(index: u32) -> ScriptedPrompt {
        let mut prompt = Self::choosing(0);
        prompt.index = index;
        prompt
    }

    pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: many.to_vec(),
            index: 0,
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
            headings: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn canceling() -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: Vec::new(),
            index: 0,
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

    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        ceiling: u32,
        maximums: &mut dyn FnMut(usize) -> Option<u32>,
    ) -> Result<(usize, u32)> {
        // 決め打ちのpromptは描画しないため、案件ごとの計算結果は使わない。
        let _ = maximums;
        self.record(heading, candidates);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok((self.one, self.index.min(ceiling)))
    }
}

impl ScriptedPrompt {
    fn record(&self, heading: &Msg, candidates: &[String]) {
        self.headings.borrow_mut().push(heading.id);
        self.asked.borrow_mut().push(candidates.to_vec());
    }
}
