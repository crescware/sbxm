use crate::diagnostics::{Diagnostic, Msg};

use crate::design::table::Table;

use crate::design::Cell;
use crate::design::text::CommandLine;

use super::{Block, Fact, Field, Guidance, GuidanceItem, LegendEntry, Section, SectionBody};

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
    pub fn warning(self, message: Msg, facts: Vec<Fact>) -> Document {
        self.push(Block::Warning { message, facts })
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
