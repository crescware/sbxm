use crate::design::Filter;
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
    /// `select_one`と`select_many`へ渡された、絞り込めるかどうか。
    pub filters: std::cell::RefCell<Vec<Filter>>,
    /// `select_open`へ渡された案件ごとの最後のindex。
    pub maximums: std::cell::RefCell<Vec<Vec<Option<u32>>>>,
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
            filters: std::cell::RefCell::new(Vec::new()),
            maximums: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 案件は先頭から選び、worktree indexは指定値で確定する。
    pub fn choosing_worktree(index: u32) -> ScriptedPrompt {
        let mut prompt = Self::choosing(0);
        prompt.index = index;
        prompt
    }

    pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
        let mut prompt = Self::choosing(0);
        prompt.many = many.to_vec();
        prompt
    }

    pub fn canceling() -> ScriptedPrompt {
        let mut prompt = Self::choosing(0);
        prompt.canceled = true;
        prompt
    }
}

impl ProjectPrompt for ScriptedPrompt {
    fn select_one(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        filter: Filter,
    ) -> Result<usize> {
        self.record(heading, candidates);
        self.filters.borrow_mut().push(filter);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.one)
    }

    fn select_many(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        filter: Filter,
    ) -> Result<Vec<usize>> {
        self.record(heading, candidates);
        self.filters.borrow_mut().push(filter);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.many.clone())
    }

    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximums: &[Option<u32>],
    ) -> Result<(usize, u32)> {
        self.record(heading, candidates);
        self.maximums.borrow_mut().push(maximums.to_vec());
        if self.canceled {
            return Err(Error::Canceled);
        }
        // 決め打ちの値をそのまま返す。範囲へ収めるのは実端末のpromptであり、範囲外の値は
        // lock後の読み直しが扱う経路をtestから踏むために使う。
        Ok((self.one, self.index))
    }
}

impl ScriptedPrompt {
    fn record(&self, heading: &Msg, candidates: &[String]) {
        self.headings.borrow_mut().push(heading.id);
        self.asked.borrow_mut().push(candidates.to_vec());
    }
}
