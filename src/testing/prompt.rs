//! 選択と入力を決め打ちするprompt。

use crate::commands::add::exec::IdentityPrompt;
use crate::error::{Error, Msg, Result};
use crate::msg;
use crate::support::select::ProjectPrompt;

/// 名義の入力を決め打ちするprompt。
///
/// 打たれた値だけでなく、初期値として置かれた候補も記録する。hostの値が決定では
/// なく候補として現れることを、testが確かめられるようにする。
pub struct ScriptedIdentityPrompt {
    typed_name: String,
    typed_email: String,
    canceled: bool,
    /// 訊かれた見出しと、そこへ置かれていた候補。
    pub offered: std::cell::RefCell<Vec<(&'static str, String)>>,
}

impl ScriptedIdentityPrompt {
    pub fn typing(name: &str, email: &str) -> ScriptedIdentityPrompt {
        ScriptedIdentityPrompt {
            typed_name: name.to_string(),
            typed_email: email.to_string(),
            canceled: false,
            offered: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 候補をそのまま確定する。利用者がEnterだけを押した場合にあたる。
    pub fn accepting_the_candidates() -> ScriptedIdentityPrompt {
        ScriptedIdentityPrompt {
            typed_name: String::new(),
            typed_email: String::new(),
            canceled: false,
            offered: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn canceling() -> ScriptedIdentityPrompt {
        ScriptedIdentityPrompt {
            typed_name: String::new(),
            typed_email: String::new(),
            canceled: true,
            offered: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// 置かれた候補。訊かれていなければ`None`。
    pub fn candidate_for(&self, heading: &str) -> Option<String> {
        self.offered
            .borrow()
            .iter()
            .find(|(asked, _)| *asked == heading)
            .map(|(_, candidate)| candidate.clone())
    }

    pub fn asked_anything(&self) -> bool {
        !self.offered.borrow().is_empty()
    }

    fn answer(&self, heading: Msg, candidate: &str, typed: &str) -> Result<String> {
        self.offered
            .borrow_mut()
            .push((heading.id, candidate.to_string()));
        if self.canceled {
            return Err(Error::Canceled);
        }
        // 何も打たなければ、初期値として置かれた候補がそのまま確定する。
        if typed.is_empty() {
            return Ok(candidate.to_string());
        }
        Ok(typed.to_string())
    }
}

impl IdentityPrompt for ScriptedIdentityPrompt {
    fn git_user_name(&mut self, candidate: &str) -> Result<String> {
        self.answer(msg!("prompt-git-user-name"), candidate, &self.typed_name)
    }

    fn git_user_email(&mut self, candidate: &str) -> Result<String> {
        self.answer(msg!("prompt-git-user-email"), candidate, &self.typed_email)
    }
}

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
    fn select_one(&mut self, heading: Msg, candidates: &[String]) -> Result<usize> {
        self.record(heading, candidates);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.one)
    }

    fn select_many(&mut self, heading: Msg, candidates: &[String]) -> Result<Vec<usize>> {
        self.record(heading, candidates);
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.many.clone())
    }
}

impl ScriptedPrompt {
    fn record(&self, heading: Msg, candidates: &[String]) {
        self.headings.borrow_mut().push(heading.id);
        self.asked.borrow_mut().push(candidates.to_vec());
    }
}
