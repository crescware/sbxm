use crate::diagnostics::{Msg, Result};

/// 削除計画を見せたあと、対象sandbox名の完全入力を1行読む。
///
/// 一致の判定はここでは行わない。呼び出し側が読み取った文字列を
/// [`super::confirmation::confirm`] へそのまま渡し、判定はそこへ一本化する。
pub trait ConfirmPrompt {
    fn read_sandbox_name(&mut self, heading: &Msg) -> Result<String>;
}
