use crate::diagnostics::{Diagnostic, Msg};

use crate::design::text::CommandLine;

use super::{Fact, Guidance, Section};

/// 出力の1単位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// 実行中の工程。連続する工程の間には空行を置かない。
    Progress(Msg),
    /// commandの結論を示す一行。
    Summary(Msg),
    /// 見出しと、fields、table、list、legendのいずれか。
    Section(Section),
    /// 補足と次の行動。commandは含まない。
    Guidance(Guidance),
    /// 結果を隠さずに伝える注意と、その説明から追い出した事実。
    Warning { message: Msg, facts: Vec<Fact> },
    /// 本文から切り離して読ませる注記。
    Note(Msg),
    /// 利用者が実行する一行。
    Command(CommandLine),
    /// 失敗1件と、その対処、外部出力。
    Diagnostic(Box<Diagnostic>),
    /// 既に組み立てられた本文。helpとversionのように、翻訳もstyleも外で決まるもの。
    Verbatim(String),
}
