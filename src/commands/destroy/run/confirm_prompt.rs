use crate::diagnostics::Result;

/// 削除確認。TTYの通常modeだけがSandbox名の完全入力を求める。
pub trait ConfirmPrompt {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool>;
}
