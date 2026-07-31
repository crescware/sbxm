use crate::diagnostics::Result;
use crate::msg;

use crate::design::PromptUi;

use super::ConfirmPrompt;

/// 共通promptで1行を読む対話実装。
///
/// yes/noでは削除しない。完全一致だけを続行の合図とするため、選択一覧ではなく
/// 自由入力を使う。EscとCtrl-Cはどちらも何も変更せず終える。
pub struct TerminalConfirmPrompt {
    prompt: PromptUi,
}

impl TerminalConfirmPrompt {
    pub fn new(prompt: PromptUi) -> TerminalConfirmPrompt {
        TerminalConfirmPrompt { prompt }
    }
}

impl ConfirmPrompt for TerminalConfirmPrompt {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool> {
        let typed = self.prompt.exact(&msg!("destroy-confirm-prompt"))?;
        Ok(typed.trim() == expected)
    }
}
