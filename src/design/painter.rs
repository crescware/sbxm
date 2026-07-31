use crate::diagnostics::{Diagnostic, ExternalFailure, Msg};
use crate::i18n::Catalog;
use crate::msg;

use crate::design::Cell;
use crate::design::policy::StreamPolicy;
use crate::design::style::{self, GlyphSet, Role, StyleSpec, VisualState};
use crate::design::table::Table;
use crate::design::text::{CommandLine, Inline};
use crate::design::width::{display_width, padding};
use crate::design::{Block, Field, GuidanceItem, LegendEntry, Section, SectionBody};

use super::{Trailing, blank, line, paint, row};

/// 列と列のあいだ。
const GAP: usize = 2;

/// 小blockの字下げ。
const INDENT: &str = "  ";

/// 外部outputの字下げ。sbxm自身の診断と視覚的に分ける。
const EXTERNAL_INDENT: &str = "    ";

/// 外部byte列がstyle stateへ侵入しないための打ち切り。
///
/// `色を出さないstreamはANSI` byteを一切生成しないという契約を優先するため、色を出す
/// streamにだけ書く。
const RESET: &str = "\u{1b}[0m";

/// 意味を文字列へ変える。streamごとの条件だけを持ち、書き出しは行わない。
pub(super) struct Painter {
    pub(super) policy: StreamPolicy,
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

    /// FTLのformatに失敗しても出力を止めず、失敗したmessage `IDとlocaleを示す`。
    fn format(catalog: &Catalog, message: &Msg) -> String {
        catalog
            .format(message)
            .unwrap_or_else(|failure| failure.to_string())
    }

    pub(crate) fn block(&self, catalog: &Catalog, block: &Block, out: &mut Vec<u8>) -> Trailing {
        match block {
            Block::Progress(message) => {
                let marker = self.role(self.glyphs().progress, Role::ProgressMarker);
                line(out, &format!("{marker} {}", Self::format(catalog, message)));
                Trailing::Normal
            }
            Block::Summary(message) => {
                let marker = self.role(self.glyphs().success, Role::SuccessMarker);
                line(out, &format!("{marker} {}", Self::format(catalog, message)));
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
                        &self.role(&Self::format(catalog, heading), Role::Heading),
                    );
                }
                for item in &guidance.items {
                    line(out, &Self::guidance_item(catalog, item));
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
        }
    }

    /// `! Warning: <message>`のように、markerとlocalized labelを添えた一行。
    fn labelled(&self, catalog: &Catalog, message: &Msg, label_id: &'static str) -> String {
        let marker = self.role(self.glyphs().warning, Role::WarningMarker);
        let label = self.role(&Self::format(catalog, &msg!(label_id)), Role::WarningMarker);
        format!("{marker} {label} {}", Self::format(catalog, message))
    }

    fn command(&self, command: &CommandLine) -> String {
        self.role(command.as_str(), Role::Command)
    }

    fn guidance_item(catalog: &Catalog, item: &GuidanceItem) -> String {
        match item {
            GuidanceItem::Ordered { number, text } => {
                format!("{INDENT}{number}. {}", Self::format(catalog, text))
            }
            GuidanceItem::Plain(text) => format!("{INDENT}{}", Self::format(catalog, text)),
        }
    }

    fn section(&self, catalog: &Catalog, section: &Section, out: &mut Vec<u8>) {
        // headingと内容のあいだに空行を置かない。離すとheadingが何を指すか弱まる。
        if let Some(heading) = &section.heading {
            line(
                out,
                &self.role(&Self::format(catalog, heading), Role::Heading),
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
                line(out, &format!("{INDENT}{}", Self::format(catalog, message)));
            }
        }
    }

    fn fields(&self, catalog: &Catalog, fields: &[Field], out: &mut Vec<u8>) {
        let labels: Vec<String> = fields
            .iter()
            .map(|field| Self::format(catalog, &field.label))
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
            .map(|header| Self::format(catalog, header))
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
                let text = Self::format(catalog, label);
                (text.clone(), text)
            }
            Cell::Value(value) => (value.as_str().to_string(), self.inline(value)),
        }
    }

    fn legend(&self, catalog: &Catalog, entries: &[LegendEntry], out: &mut Vec<u8>) {
        let width = column_width(entries.iter().map(|entry| entry.value.as_str()));
        for entry in entries {
            let described = self.role(&Self::format(catalog, &entry.description), Role::Muted);
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
            &format!("{INDENT}{}", Self::format(catalog, &diagnostic.description)),
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
                            &Self::format(catalog, &msg!("remediation-heading")),
                            Role::Heading
                        )
                    ),
                );
                for explanation in &remediation.explanation {
                    line(
                        out,
                        &format!("{EXTERNAL_INDENT}{}", Self::format(catalog, explanation)),
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
        let heading = Self::format(
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
            self.role(&Self::format(catalog, &msg!(id)), Role::Heading)
        )
    }
}

/// 列の幅。装飾を含まない元の値から数える。
fn column_width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(display_width).max().unwrap_or(0) + GAP
}

/// byte列を、行ごとに字下げして書く。
///
/// 末尾に改行がなくてもblockは改行で閉じる。`reset`は各行の前後へ挟み、原文のstyleが
/// 次の行やsbxm自身の出力へ残らないようにする。
fn indented_bytes(out: &mut Vec<u8>, raw: &[u8], indent: &str, reset: &str) {
    let mut remaining = raw;
    while !remaining.is_empty() {
        let (line, tail) = match remaining.iter().position(|byte| *byte == b'\n') {
            Some(index) => (&remaining[..index], &remaining[index + 1..]),
            None => (remaining, &remaining[remaining.len()..]),
        };
        out.extend_from_slice(indent.as_bytes());
        out.extend_from_slice(reset.as_bytes());
        out.extend_from_slice(line);
        out.extend_from_slice(reset.as_bytes());
        out.push(b'\n');
        remaining = tail;
    }
}
