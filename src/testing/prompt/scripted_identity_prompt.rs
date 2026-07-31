use crate::commands::add::IdentityPrompt;
use crate::diagnostics::{Error, Msg, Result};
use crate::msg;

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

    fn answer(&self, heading: &Msg, candidate: &str, typed: &str) -> Result<String> {
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
        self.answer(&msg!("prompt-git-user-name"), candidate, &self.typed_name)
    }

    fn git_user_email(&mut self, candidate: &str) -> Result<String> {
        self.answer(&msg!("prompt-git-user-email"), candidate, &self.typed_email)
    }
}
