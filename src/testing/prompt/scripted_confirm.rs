use crate::diagnostics::{Error, Msg, Result};
use crate::support::protection::ConfirmPrompt;

/// 入力を決め打ちする確認prompt。`None`はEscまたはCtrl-C。
pub struct ScriptedConfirm {
    typed: Option<String>,
    asked: usize,
}

impl ScriptedConfirm {
    pub fn typing(value: &str) -> ScriptedConfirm {
        ScriptedConfirm {
            typed: Some(value.to_string()),
            asked: 0,
        }
    }

    pub fn canceling() -> ScriptedConfirm {
        ScriptedConfirm {
            typed: None,
            asked: 0,
        }
    }

    /// 入力を求められた回数。
    pub fn asked(&self) -> usize {
        self.asked
    }
}

impl ConfirmPrompt for ScriptedConfirm {
    fn read_sandbox_name(&mut self, _heading: &Msg) -> Result<String> {
        self.asked += 1;
        match &self.typed {
            Some(typed) => Ok(typed.clone()),
            None => Err(Error::Canceled),
        }
    }
}
