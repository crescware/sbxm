use crate::diagnostics::Msg;

use crate::design::CommandLine;

/// toolが利用者へ返す案内。
///
/// sbxmが代わりに実行しないことを示すために使う。errorではないため、stdoutへ出す。
///
/// 実行を求めるcommandは説明文へ埋め込まず、typedな一行として持つ。rendererが独立した
/// blockとして描き、利用者はそのまま複写できる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub heading: Msg,
    /// 案内の対象。pathや識別子であり、翻訳しない。
    pub items: Vec<String>,
    pub hint: Msg,
    /// 利用者が自分で実行するcommand。
    pub commands: Vec<CommandLine>,
}
