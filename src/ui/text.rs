//! 翻訳しない短い値と、実行を指示する一行。
//!
//! 翻訳済みの一文字列へsubstring検索で部分装飾を当てない。装飾の判断は文字列を組み立てる
//! 前に型で決めておき、rendererはその型だけを見る。

use super::style::VisualState;

/// 翻訳しない短い値。装飾の判断を型で持つ。
///
/// pathやIDをtableの行ごとにboldにするとノイズになるため、既定は端末の既定色である。
/// 照合の基準になる値だけを[`Inline::Important`]で示す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// 端末の既定色のまま出す値。
    Text(String),
    /// 照合の基準になる短い値。project ID、sandbox名、error IDなど。
    Important(String),
    /// host上のpath。行ごとの強調はしない。
    Path(String),
    /// 状態値。文脈が決めたsemantic stateで着色する。
    State { text: String, state: VisualState },
}

impl Inline {
    pub fn text(value: impl Into<String>) -> Inline {
        Inline::Text(value.into())
    }

    pub fn important(value: impl Into<String>) -> Inline {
        Inline::Important(value.into())
    }

    pub fn path(value: impl Into<String>) -> Inline {
        Inline::Path(value.into())
    }

    pub fn state(value: impl Into<String>, state: VisualState) -> Inline {
        Inline::State {
            text: value.into(),
            state,
        }
    }

    /// 装飾を除いた元の文字列。列幅はこの値から数える。
    pub fn as_str(&self) -> &str {
        match self {
            Inline::Text(value) | Inline::Important(value) | Inline::Path(value) => value,
            Inline::State { text, .. } => text,
        }
    }
}

/// [`CommandLine::new`]が受け付けなかった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidCommandLine {
    /// 空の指示は「何を実行するか」を示さない。
    Empty,
    /// 改行を含む手順は一行のcommandではない。
    Multiline,
}

impl std::fmt::Display for InvalidCommandLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidCommandLine::Empty => "a command line cannot be empty",
            InvalidCommandLine::Multiline => "a command line cannot span more than one line",
        })
    }
}

/// 利用者がそのままshellへ入力する一行。
///
/// 説明文との混在を型で防ぎ、rendererが前後の空行を保証する。色やboldだけでは本文との
/// 境界にならないため、色を消しても空行で区別できることを不変条件とする。
///
/// secretを含まないことは生成時点の責務であり、この型はsanitizerではない。redactは
/// `support::secret`のように値を組み立てる側が行う。
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

impl std::fmt::Display for CommandLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

#[cfg(test)]
#[path = "text_test.rs"]
mod text_test;
