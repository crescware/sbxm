//! 出力の構造。
//!
//! command printerは何をどの順で見せるかだけを宣言し、空行、prefix、字下げ、ANSIを
//! 組み立てない。`println!("\n...")`で余白を作ると、同じ意味のblockでも間隔がcommandごとに
//! ずれる。blockの境界はrendererが1箇所で管理する。
//!
//! blockの中は詰め、block間へ空行を一行置く。builderは空のsectionを黙って落とすため、
//! 「対象ゼロ」を見せたい場合だけ[`Document::empty_section`]で明示する。

use crate::error::{Diagnostic, Msg};

use super::table::{Cell, Table};
use super::text::{CommandLine, Inline};

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
    /// 結果を隠さずに伝える注意。
    Warning(Msg),
    /// 本文から切り離して読ませる注記。
    Note(Msg),
    /// 利用者が実行する一行。
    Command(CommandLine),
    /// 失敗1件と、その対処、外部出力。
    Diagnostic(Box<Diagnostic>),
    /// 既に組み立てられた本文。helpとversionのように、翻訳もstyleも外で決まるもの。
    Verbatim(String),
}

/// 見出しと内容。見出しと内容の間には空行を置かない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: Option<Msg>,
    pub body: SectionBody,
}

/// sectionが並べるもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionBody {
    /// 項目名と値の一覧。
    Fields(Vec<Field>),
    /// 列をそろえた一覧。
    Table(Table),
    /// 1行1件の並び。翻訳する説明と翻訳しない値が混在し得る。
    Lines(Vec<Cell>),
    /// 出現した状態値とその説明。
    Legend(Vec<LegendEntry>),
    /// 対象がゼロであること自体が結果である場合の一行。
    Empty(Msg),
}

/// 項目名と値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub label: Msg,
    pub value: Inline,
}

impl Field {
    pub fn new(label: Msg, value: Inline) -> Field {
        Field { label, value }
    }
}

/// 状態値と、その説明。
///
/// 状態値は翻訳しない安定した文字列であり、説明だけを訳す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    pub value: String,
    pub description: Msg,
}

impl LegendEntry {
    pub fn new(value: impl Into<String>, description: Msg) -> LegendEntry {
        LegendEntry {
            value: value.into(),
            description,
        }
    }
}

/// 補足と次の行動。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guidance {
    pub heading: Option<Msg>,
    pub items: Vec<GuidanceItem>,
}

/// guidanceの1行。
///
/// 番号とbulletはこの行に付く。command行へは決して付かない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuidanceItem {
    /// 順序のある操作。
    Ordered { number: usize, text: Msg },
    /// markerを付けない説明。
    Plain(Msg),
}

/// 1回の出力。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    blocks: Vec<Block>,
}

impl Document {
    pub fn new() -> Document {
        Document::default()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// blockを1つ足す。builderが表現できない組み合わせを作らないよう、公開しない。
    fn push(mut self, block: Block) -> Document {
        self.blocks.push(block);
        self
    }

    /// 実行中の工程。
    pub fn progress(self, message: Msg) -> Document {
        self.push(Block::Progress(message))
    }

    /// 結論の一行。
    pub fn summary(self, message: Msg) -> Document {
        self.push(Block::Summary(message))
    }

    /// 項目名と値の一覧。1件もなければsectionごと省く。
    pub fn fields(self, heading: Option<Msg>, fields: Vec<Field>) -> Document {
        if fields.is_empty() {
            return self;
        }
        self.section(heading, SectionBody::Fields(fields))
    }

    /// 列をそろえた一覧。行がなければsectionごと省く。
    pub fn table(self, heading: Option<Msg>, table: Table) -> Document {
        if table.is_empty() {
            return self;
        }
        self.section(heading, SectionBody::Table(table))
    }

    /// 1行1件の並び。1件もなければsectionごと省く。
    pub fn lines(self, heading: Option<Msg>, lines: Vec<Cell>) -> Document {
        if lines.is_empty() {
            return self;
        }
        self.section(heading, SectionBody::Lines(lines))
    }

    /// 出現した状態値の凡例。1件もなければsectionごと省く。
    pub fn legend(self, heading: Msg, entries: Vec<LegendEntry>) -> Document {
        if entries.is_empty() {
            return self;
        }
        self.section(Some(heading), SectionBody::Legend(entries))
    }

    /// 対象がゼロであること自体を結果として示すsection。
    pub fn empty_section(self, heading: Option<Msg>, message: Msg) -> Document {
        self.section(heading, SectionBody::Empty(message))
    }

    fn section(self, heading: Option<Msg>, body: SectionBody) -> Document {
        self.push(Block::Section(Section { heading, body }))
    }

    /// 補足と次の行動。1件もなければblockごと省く。
    pub fn guidance(self, heading: Option<Msg>, items: Vec<GuidanceItem>) -> Document {
        if items.is_empty() && heading.is_none() {
            return self;
        }
        self.push(Block::Guidance(Guidance { heading, items }))
    }

    /// 結果を隠さずに伝える注意。
    ///
    /// 成功を打ち消さないよう、summaryとは別blockのまま両方を残す。
    pub fn warning(self, message: Msg) -> Document {
        self.push(Block::Warning(message))
    }

    /// 本文から切り離して読ませる注記。
    pub fn note(self, message: Msg) -> Document {
        self.push(Block::Note(message))
    }

    /// 利用者が実行する一行。常に独立blockになる。
    pub fn command(self, command: CommandLine) -> Document {
        self.push(Block::Command(command))
    }

    /// commandを組み立てられた場合だけblockを足す。
    pub fn try_command(self, command: impl Into<String>) -> Document {
        match CommandLine::optional(command) {
            Some(command) => self.command(command),
            None => self,
        }
    }

    /// 失敗1件。
    pub fn diagnostic(self, diagnostic: Diagnostic) -> Document {
        self.push(Block::Diagnostic(Box::new(diagnostic)))
    }

    /// 既に組み立てられた本文。
    ///
    /// helpとversionだけが使う。末尾の改行はrendererが1つに揃えるため、呼び出し側で
    /// 空行を作らない。
    pub fn verbatim(self, text: impl Into<String>) -> Document {
        self.push(Block::Verbatim(text.into()))
    }

    /// 別のdocumentを末尾へ連結する。blockの順序は保たれる。
    pub fn concat(mut self, other: Document) -> Document {
        self.blocks.extend(other.blocks);
        self
    }
}

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
