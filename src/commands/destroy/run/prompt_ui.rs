use crate::design::PromptUi;
use crate::diagnostics::Result;
use crate::msg;

use super::ConfirmPrompt;

/// 共通promptで1行を読む対話実装。
///
/// yes/noでは削除しない。完全一致だけを続行の合図とするため、選択一覧ではなく
/// 自由入力を使う。EscとCtrl-Cはどちらも何も変更せず終える。
///
/// 確認だけの型を挟まないのは、名義と案件を訊く2つと同じく、promptの実装は`design`が
/// 1つ持てば足りるためである。
impl ConfirmPrompt for PromptUi {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool> {
        let typed = self.exact(&msg!("destroy-confirm-prompt"))?;
        Ok(typed.trim() == expected)
    }
}

#[cfg(test)]
#[path = "prompt_ui_test.rs"]
mod prompt_ui_test;
