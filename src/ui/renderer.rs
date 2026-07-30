//! blockをstreamへ描く。
//!
//! ANSI escape sequenceを生成するのは本fileだけとする。ほかのmoduleは意味だけを宣言し、
//! 色の有無、glyphの選択、空行の位置を知らない。生成箇所を1つに保つと、色なし出力に
//! escape byteが混ざらないことをbyte単位で確かめられる。
//!
//! block間の空行も本fileが数える。callerが文字列の先頭へ`\n`を書くと、同じ意味のblockでも
//! commandごとに間隔がずれ、先頭空行や三連続改行が混ざる。

use std::io::Write;

use crate::error::{Diagnostic, ExternalFailure, Msg};
use crate::i18n::Catalog;
use crate::msg;

use super::document::{Block, Document, Field, GuidanceItem, LegendEntry, Section, SectionBody};
use super::policy::StreamPolicy;
use super::style::{self, Color, GlyphSet, Role, StyleSpec, VisualState};
use super::table::{Cell, Table};
use super::text::{CommandLine, Inline};
use super::width::{display_width, padding};

/// 列と列のあいだ。
const GAP: usize = 2;

/// 小blockの字下げ。
const INDENT: &str = "  ";

/// 外部outputの字下げ。sbxm自身の診断と視覚的に分ける。
const EXTERNAL_INDENT: &str = "    ";

/// 外部byte列がstyle stateへ侵入しないための打ち切り。
///
/// 色を出さないstreamはANSI byteを一切生成しないという契約を優先するため、色を出す
/// streamにだけ書く。
const RESET: &str = "\u{1b}[0m";

/// blockのあとに空行が要るか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trailing {
    /// 次のblockが来るときに空行を置く。
    Normal,
    /// 直ちに空行で閉じる。command行の直後は必ず空行になる。
    Blank,
}

/// 1 streamへの描画。
pub struct Renderer<'a> {
    writer: Box<dyn Write + 'a>,
    painter: Painter,
    /// 次に何かを書く前に空行が要るか。
    blank_owed: bool,
    /// 直前に書いたのが工程行か。工程は連続させ、あいだに空行を置かない。
    last_was_progress: bool,
}

impl<'a> Renderer<'a> {
    pub fn new(writer: impl Write + 'a, policy: StreamPolicy) -> Renderer<'a> {
        Renderer {
            writer: Box::new(writer),
            painter: Painter { policy },
            blank_owed: false,
            last_was_progress: false,
        }
    }

    pub(super) fn policy(&self) -> StreamPolicy {
        self.painter.policy
    }

    /// promptのように、rendererを経由せず同じstreamへ書いたことを知らせる。
    ///
    /// 次のblockが空行から始まるようにするためだけに使う。
    pub(super) fn note_external_write(&mut self) {
        self.blank_owed = true;
        self.last_was_progress = false;
    }

    /// documentを描き、flushする。
    pub fn write(&mut self, catalog: &Catalog, document: &Document) {
        for block in document.blocks() {
            let progress = matches!(block, Block::Progress(_));
            self.separate(progress);

            let mut buffer = Vec::new();
            let trailing = self.painter.block(catalog, block, &mut buffer);
            let _ = self.writer.write_all(&buffer);

            self.blank_owed = true;
            self.last_was_progress = progress;
            if trailing == Trailing::Blank {
                let _ = self.writer.write_all(b"\n");
                self.blank_owed = false;
            }
        }
        let _ = self.writer.flush();
    }

    /// blockの前に空行を置く。
    fn separate(&mut self, progress: bool) {
        if self.blank_owed && !(progress && self.last_was_progress) {
            let _ = self.writer.write_all(b"\n");
        }
        self.blank_owed = false;
    }
}

/// 意味を文字列へ変える。streamごとの条件だけを持ち、書き出しは行わない。
struct Painter {
    policy: StreamPolicy,
}

impl Painter {
    fn glyphs(&self) -> GlyphSet {
        style::glyphs(self.policy.characters)
    }

    /// 装飾を載せる。色を出さないstreamでは元の文字列をそのまま返す。
    fn paint(&self, text: &str, spec: StyleSpec) -> String {
        if !self.policy.color {
            return text.to_string();
        }
        paint(text, spec)
    }

    fn role(&self, text: &str, role: Role) -> String {
        self.paint(text, style::role_style(role))
    }

    fn state(&self, text: &str, state: VisualState) -> String {
        self.paint(text, style::state_style(state))
    }

    /// typed fragmentを装飾する。
    fn inline(&self, value: &Inline) -> String {
        match value {
            Inline::Text(text) | Inline::Path(text) => text.clone(),
            Inline::Important(text) => self.role(text, Role::Important),
            Inline::State { text, state } => self.state(text, *state),
        }
    }

    /// FTLのformatに失敗しても出力を止めず、失敗したmessage IDとlocaleを示す。
    fn format(&self, catalog: &Catalog, message: &Msg) -> String {
        catalog
            .format(message)
            .unwrap_or_else(|failure| failure.to_string())
    }

    fn block(&self, catalog: &Catalog, block: &Block, out: &mut Vec<u8>) -> Trailing {
        match block {
            Block::Progress(message) => {
                let marker = self.role(self.glyphs().progress, Role::ProgressMarker);
                line(out, &format!("{marker} {}", self.format(catalog, message)));
                Trailing::Normal
            }
            Block::Summary(message) => {
                let marker = self.role(self.glyphs().success, Role::SuccessMarker);
                line(out, &format!("{marker} {}", self.format(catalog, message)));
                Trailing::Normal
            }
            Block::Section(section) => {
                self.section(catalog, section, out);
                Trailing::Normal
            }
            Block::Guidance(guidance) => {
                if let Some(heading) = &guidance.heading {
                    line(
                        out,
                        &self.role(&self.format(catalog, heading), Role::Heading),
                    );
                }
                for item in &guidance.items {
                    line(out, &self.guidance_item(catalog, item));
                }
                Trailing::Normal
            }
            Block::Warning(message) => {
                line(out, &self.labelled(catalog, message, "warning-label"));
                Trailing::Normal
            }
            Block::Note(message) => {
                line(out, &self.labelled(catalog, message, "note-label"));
                Trailing::Normal
            }
            Block::Command(command) => {
                line(out, &self.command(command));
                // 本文との境界を色ではなく空行で作る。色なしでも区別できる必要がある。
                Trailing::Blank
            }
            Block::Diagnostic(diagnostic) => self.diagnostic(catalog, diagnostic, out),
            Block::Verbatim(text) => {
                // 末尾の改行はrendererが1つに揃える。callerが空行で余白を作らない。
                for text in text.trim_end_matches('\n').split('\n') {
                    line(out, text);
                }
                Trailing::Normal
            }
            Block::Rule => {
                let width = self.policy.width.unwrap_or(72).clamp(8, 72);
                line(
                    out,
                    &self.role(&self.glyphs().horizontal_rule.repeat(width), Role::Muted),
                );
                Trailing::Normal
            }
        }
    }

    /// `! Warning: <message>`のように、markerとlocalized labelを添えた一行。
    fn labelled(&self, catalog: &Catalog, message: &Msg, label_id: &'static str) -> String {
        let marker = self.role(self.glyphs().warning, Role::WarningMarker);
        let label = self.role(&self.format(catalog, &msg!(label_id)), Role::WarningMarker);
        format!("{marker} {label} {}", self.format(catalog, message))
    }

    fn command(&self, command: &CommandLine) -> String {
        self.role(command.as_str(), Role::Command)
    }

    fn guidance_item(&self, catalog: &Catalog, item: &GuidanceItem) -> String {
        match item {
            GuidanceItem::Ordered { number, text } => {
                format!("{INDENT}{number}. {}", self.format(catalog, text))
            }
            GuidanceItem::Bullet(text) => format!("{INDENT}- {}", self.format(catalog, text)),
            GuidanceItem::Plain(text) => format!("{INDENT}{}", self.format(catalog, text)),
        }
    }

    fn section(&self, catalog: &Catalog, section: &Section, out: &mut Vec<u8>) {
        // headingと内容のあいだに空行を置かない。離すとheadingが何を指すか弱まる。
        if let Some(heading) = &section.heading {
            line(
                out,
                &self.role(&self.format(catalog, heading), Role::Heading),
            );
        }
        match &section.body {
            SectionBody::Fields(fields) => self.fields(catalog, fields, out),
            SectionBody::Table(table) => self.table(catalog, table, out),
            SectionBody::Lines(lines) => {
                for value in lines {
                    let (_, painted) = self.cell(catalog, value);
                    line(out, &format!("{INDENT}{painted}"));
                }
            }
            SectionBody::Legend(entries) => self.legend(catalog, entries, out),
            SectionBody::Empty(message) => {
                line(out, &format!("{INDENT}{}", self.format(catalog, message)));
            }
        }
    }

    fn fields(&self, catalog: &Catalog, fields: &[Field], out: &mut Vec<u8>) {
        let labels: Vec<String> = fields
            .iter()
            .map(|field| self.format(catalog, &field.label))
            .collect();
        let width = column_width(labels.iter().map(String::as_str));
        for (label, field) in labels.iter().zip(fields) {
            // 幅は装飾前のlabelから数え、余白は装飾の外側へ置く。
            let padded = format!("{}{}", self.role(label, Role::Muted), padding(label, width));
            line(out, &format!("{padded}{}", self.inline(&field.value)));
        }
    }

    fn table(&self, catalog: &Catalog, table: &Table, out: &mut Vec<u8>) {
        let columns = table.columns();
        let headers: Vec<String> = table
            .headers()
            .iter()
            .map(|header| self.format(catalog, header))
            .collect();

        // 幅は装飾前の値から数える。行の描画は2度目のformatを避けるため先に済ませる。
        let painted: Vec<Vec<(String, String)>> = table
            .rows()
            .iter()
            .map(|cells| cells.iter().map(|cell| self.cell(catalog, cell)).collect())
            .collect();

        let widths: Vec<usize> = (0..columns)
            .map(|column| {
                column_width(
                    painted
                        .iter()
                        .filter_map(|row| row.get(column))
                        .map(|(plain, _)| plain.as_str())
                        .chain(headers.get(column).map(String::as_str)),
                )
            })
            .collect();

        let header_cells: Vec<(String, String)> = headers
            .iter()
            .map(|header| (header.clone(), self.role(header, Role::TableHeader)))
            .collect();
        line(out, &row(&header_cells, &widths));

        for cells in &painted {
            line(out, &row(cells, &widths));
        }
    }

    /// cellの`(装飾前, 装飾後)`。
    fn cell(&self, catalog: &Catalog, cell: &Cell) -> (String, String) {
        match cell {
            Cell::Label(label) => {
                let text = self.format(catalog, label);
                (text.clone(), text)
            }
            Cell::Value(value) => (value.as_str().to_string(), self.inline(value)),
        }
    }

    fn legend(&self, catalog: &Catalog, entries: &[LegendEntry], out: &mut Vec<u8>) {
        let width = column_width(entries.iter().map(|entry| entry.value.as_str()));
        for entry in entries {
            let described = self.role(&self.format(catalog, &entry.description), Role::Muted);
            line(
                out,
                &format!(
                    "{INDENT}{}{}{described}",
                    entry.value,
                    padding(&entry.value, width)
                ),
            );
        }
    }

    fn diagnostic(
        &self,
        catalog: &Catalog,
        diagnostic: &Diagnostic,
        out: &mut Vec<u8>,
    ) -> Trailing {
        let marker = self.role(self.glyphs().error, Role::ErrorMarker);
        // error IDは翻訳しない安定した英語であり、prefixもrendererが付ける。
        let label = self.role("error:", Role::ErrorMarker);
        let id = self.role(diagnostic.id.as_str(), Role::Important);
        line(out, &format!("{marker} {label} {id}"));
        line(
            out,
            &format!("{INDENT}{}", self.format(catalog, &diagnostic.description)),
        );

        let mut trailing = Trailing::Normal;
        if let Some(remediation) = &diagnostic.remediation {
            if !remediation.explanation.is_empty() {
                blank(out);
                line(
                    out,
                    &format!(
                        "{INDENT}{}",
                        self.role(
                            &self.format(catalog, &msg!("remediation-heading")),
                            Role::Heading
                        )
                    ),
                );
                for explanation in &remediation.explanation {
                    line(
                        out,
                        &format!("{EXTERNAL_INDENT}{}", self.format(catalog, explanation)),
                    );
                }
                trailing = Trailing::Normal;
            }
            for command in &remediation.commands {
                blank(out);
                line(out, &self.command(command));
                trailing = Trailing::Blank;
            }
        }

        if let Some(external) = &diagnostic.external {
            self.external(catalog, external, out);
            trailing = Trailing::Normal;
        }
        trailing
    }

    fn external(&self, catalog: &Catalog, external: &ExternalFailure, out: &mut Vec<u8>) {
        blank(out);
        line(
            out,
            &self.small_heading(catalog, "external-invocation-heading"),
        );
        blank(out);
        // 失敗した工程を同じ形で読めるよう、起動そのものを一行で示す。実行指示ではないが、
        // 診断のなかでは同じ視認性を持たせる。
        let invocation = format!("{} {}", external.program, external.safe_args.join(" "));
        line(out, &self.role(invocation.trim_end(), Role::Command));

        if let Some(directory) = &external.working_dir {
            blank(out);
            line(
                out,
                &self.small_heading(catalog, "external-directory-heading"),
            );
            line(
                out,
                &format!("{EXTERNAL_INDENT}{}", crate::paths::display(directory)),
            );
        }

        if external.stderr_lossy {
            blank(out);
            line(
                out,
                &self.labelled(
                    catalog,
                    &msg!(
                        "warning-external-output-lossy",
                        program = external.program,
                        stream = "stderr"
                    ),
                    "warning-label",
                ),
            );
        }

        if external.stderr.is_empty() {
            return;
        }
        blank(out);
        let heading = self.format(
            catalog,
            &msg!("external-output-heading", program = external.program),
        );
        line(
            out,
            &format!("{INDENT}{}", self.role(&heading, Role::Heading)),
        );
        // 外部byte列は翻訳も着色もせず、字下げだけを足して原文のまま出す。原文に残っていた
        // escape sequenceがrendererのstyleへ侵入しないよう、行ごとに前後で打ち切る。
        // 色を出さないstreamはescape byteを一切書かないため、打ち切りも行わない。
        let reset = if self.policy.color { RESET } else { "" };
        indented_bytes(out, &external.stderr, EXTERNAL_INDENT, reset);
    }

    fn small_heading(&self, catalog: &Catalog, id: &'static str) -> String {
        format!(
            "{INDENT}{}",
            self.role(&self.format(catalog, &msg!(id)), Role::Heading)
        )
    }
}

/// 装飾を載せる。ANSIを組み立てる唯一の入口として、promptもここを通す。
pub(super) fn paint(text: &str, spec: StyleSpec) -> String {
    if spec.is_plain() || text.is_empty() {
        return text.to_string();
    }
    console_style(spec).apply_to(text).to_string()
}

/// 1行を書く。行は必ず改行で閉じる。
fn line(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(b'\n');
}

fn blank(out: &mut Vec<u8>) {
    out.push(b'\n');
}

/// 列の幅。装飾を含まない元の値から数える。
fn column_width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(display_width).max().unwrap_or(0) + GAP
}

/// 1行分のcellを、列幅にそろえて並べる。末尾の余白は残さない。
///
/// cellは`(装飾前, 装飾後)`で受け取る。余白は装飾前の幅から決めるため、色のon/offで
/// 列の開始位置が変わらない。
fn row(cells: &[(String, String)], widths: &[usize]) -> String {
    let mut out = String::new();
    for (index, (plain, painted)) in cells.iter().enumerate() {
        out.push_str(painted);
        if index + 1 < cells.len() {
            out.push_str(&padding(plain, widths.get(index).copied().unwrap_or(0)));
        }
    }
    out
}

/// byte列を、行ごとに字下げして書く。
///
/// 末尾に改行がなくてもblockは改行で閉じる。`reset`は各行の前後へ挟み、原文のstyleが
/// 次の行やsbxm自身の出力へ残らないようにする。
fn indented_bytes(out: &mut Vec<u8>, raw: &[u8], indent: &str, reset: &str) {
    let mut rest = raw;
    while !rest.is_empty() {
        let (line, tail) = match rest.iter().position(|byte| *byte == b'\n') {
            Some(index) => (&rest[..index], &rest[index + 1..]),
            None => (rest, &rest[rest.len()..]),
        };
        out.extend_from_slice(indent.as_bytes());
        out.extend_from_slice(reset.as_bytes());
        out.extend_from_slice(line);
        out.extend_from_slice(reset.as_bytes());
        out.push(b'\n');
        rest = tail;
    }
}

/// 意味からterminal crateのstyleへ変える唯一の場所。
///
/// 標準themeはANSI named colorだけを使い、RGBや256色indexへ昇格しない。
fn console_style(spec: StyleSpec) -> console::Style {
    let mut style = console::Style::new().force_styling(true);
    if spec.bold {
        style = style.bold();
    }
    if spec.dim {
        style = style.dim();
    }
    if spec.underline {
        style = style.underlined();
    }
    if let Some(foreground) = spec.foreground {
        style = style.fg(match foreground {
            Color::Red => console::Color::Red,
            Color::Green => console::Color::Green,
            Color::Yellow => console::Color::Yellow,
            Color::Cyan => console::Color::Cyan,
        });
    }
    style
}

#[cfg(test)]
#[path = "renderer_test.rs"]
mod renderer_test;
