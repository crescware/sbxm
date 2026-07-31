use crate::diagnostics::Result;

/// 名義を1行ずつ訊くprompt。
///
/// 候補は初期値として置くだけで、確定した値ではない。EscとCtrl-Cはどちらも何も
/// 登録せず終える。
pub trait IdentityPrompt {
    fn git_user_name(&mut self, candidate: &str) -> Result<String>;
    fn git_user_email(&mut self, candidate: &str) -> Result<String>;
}
