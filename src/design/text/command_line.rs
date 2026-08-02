use super::InvalidCommandLine;

/// 利用者がそのままshellへ入力する一行。
///
/// 説明文との混在を型で防ぎ、rendererが前後の空行を保証する。色やboldだけでは本文との
/// 境界にならないため、色を消しても空行で区別できることを不変条件とする。
///
/// secretを含まないことは生成時点の責務であり、この型はsanitizerではない。redactは
/// `support::secret`のように値を組み立てる側が行う。
///
/// `Display`を実装しない。`format!`などの暗黙の文字列化で、rendererを通らない文への
/// 埋め込みを容易にしないためである。描画は`as_str`を受け取るpainterが担当する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    value: String,
}

impl CommandLine {
    /// 一行のcommandを作る。
    ///
    /// 空文字、LF、CRを拒否することで、複数行のsnippetや説明付きの行を表現できなくする。
    pub fn new(value: impl Into<String>) -> Result<CommandLine, InvalidCommandLine> {
        let value = value.into();
        if value.contains('\n') || value.contains('\r') {
            return Err(InvalidCommandLine::Multiline);
        }
        if value.trim().is_empty() {
            return Err(InvalidCommandLine::Empty);
        }
        Ok(CommandLine { value })
    }

    /// 組み立てに失敗した場合も出力を止めない。
    ///
    /// commandを見せられないことは利用者の操作を止める理由にならないため、拒否された値は
    /// commandとして表示せず、呼び出し側がblockを省く。
    pub fn optional(value: impl Into<String>) -> Option<CommandLine> {
        CommandLine::new(value).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}
