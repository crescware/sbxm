use crate::diagnostics::{Error, Msg, Result};
use crate::support::protection::ConfirmPrompt;

/// 入力を決め打ちする確認prompt。`None`はEscまたはCtrl-C。
pub struct ScriptedConfirm {
    typed: Option<Vec<String>>,
    asked: usize,
}

impl ScriptedConfirm {
    pub fn typing(value: &str) -> ScriptedConfirm {
        ScriptedConfirm {
            typed: Some(vec![value.to_string()]),
            asked: 0,
        }
    }

    /// 打ち直しを含む入力。最後の1件は繰り返し使う。
    pub fn typing_in_turn(values: &[&str]) -> ScriptedConfirm {
        ScriptedConfirm {
            typed: Some(values.iter().map(|value| (*value).to_string()).collect()),
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
    fn read_confirmation(&mut self, _heading: &Msg) -> Result<String> {
        self.asked += 1;
        match &mut self.typed {
            Some(typed) => {
                if typed.len() > 1 {
                    Ok(typed.remove(0))
                } else {
                    Ok(typed.first().cloned().unwrap_or_default())
                }
            }
            None => Err(Error::Canceled),
        }
    }
}
