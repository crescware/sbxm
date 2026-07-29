//! 利用者向け出力の組み立て。
//!
//! stdoutは正常結果に使用する。状態値は翻訳しないため、正本locale以外ではtableへ
//! 状態値の凡例を加える。stderrはprompt、warning、errorに使用する。

use std::io::Write;

use crate::error::{Diagnostic, Error, Msg};
use crate::i18n::Catalog;

use super::status::{Row, StatusValue};
use super::width::{display_width, pad_to, render_row};

/// 利用者向け出力を組み立てる。
pub struct Reporter<'a> {
    catalog: &'a Catalog,
}

impl<'a> Reporter<'a> {
    pub fn new(catalog: &'a Catalog) -> Reporter<'a> {
        Reporter { catalog }
    }

    /// FTLのformatに失敗した場合でも、失敗したmessage IDとlocaleが分かる形で返す。
    fn format(&self, message: &Msg) -> String {
        match self.catalog.format(message) {
            Ok(text) => text,
            Err(failure) => failure.to_string(),
        }
    }

    fn text(&self, id: &str) -> String {
        match self.catalog.text(id) {
            Ok(text) => text,
            Err(failure) => failure.to_string(),
        }
    }

    /// section名、列名、項目名を翻訳し、状態値は翻訳しないtableを描画する。
    pub fn render_status_table(
        &self,
        section_id: &str,
        item_header_id: &str,
        status_header_id: &str,
        rows: &[Row],
    ) -> String {
        let item_header = self.text(item_header_id);
        let labels: Vec<String> = rows.iter().map(|row| self.text(row.item)).collect();

        let width = labels
            .iter()
            .map(|label| display_width(label))
            .chain(std::iter::once(display_width(&item_header)))
            .max()
            .unwrap_or(0)
            + 2;

        let mut out = String::new();
        out.push_str(&self.text(section_id));
        out.push('\n');
        out.push_str(&pad_to(&item_header, width));
        out.push_str(&self.text(status_header_id));
        out.push('\n');
        for (label, row) in labels.iter().zip(rows) {
            out.push_str(&pad_to(label, width));
            out.push_str(row.status.as_str());
            out.push('\n');
        }
        out
    }

    /// 項目名を翻訳し、値は翻訳しない一覧。
    ///
    /// path、識別子、状態値のような、翻訳すると別のものになる値を並べるために使う。
    pub fn render_fields(&self, fields: &[(&str, String)]) -> String {
        let labels: Vec<String> = fields.iter().map(|(item, _)| self.text(item)).collect();
        let width = labels
            .iter()
            .map(|label| display_width(label))
            .max()
            .unwrap_or(0)
            + 2;

        let mut out = String::new();
        for (label, (_, value)) in labels.iter().zip(fields) {
            out.push_str(&pad_to(label, width));
            out.push_str(value);
            out.push('\n');
        }
        out
    }

    /// 列名を翻訳し、値は翻訳しないtable。
    pub fn render_value_table(&self, headers: &[&str], rows: &[Vec<String>]) -> String {
        let headers: Vec<String> = headers.iter().map(|header| self.text(header)).collect();
        let widths: Vec<usize> = (0..headers.len())
            .map(|column| {
                rows.iter()
                    .filter_map(|row| row.get(column))
                    .map(|value| display_width(value))
                    .chain(std::iter::once(display_width(&headers[column])))
                    .max()
                    .unwrap_or(0)
                    + 2
            })
            .collect();

        let mut out = String::new();
        out.push_str(&render_row(&headers, &widths));
        for row in rows {
            out.push_str(&render_row(row, &widths));
        }
        out
    }

    /// 実際に出現したenumだけの凡例。
    ///
    /// 状態値は翻訳しないため、正本locale以外の正常出力へ注釈として付ける。
    pub fn render_legend(&self, rows: &[Row]) -> Option<String> {
        let mut seen: Vec<StatusValue> = rows.iter().map(|row| row.status).collect();
        seen.sort();
        seen.dedup();
        let values: Vec<(&str, &str)> = seen
            .iter()
            .map(|status| (status.as_str(), status.legend_id()))
            .collect();
        self.render_value_legend(&values)
    }

    /// 出現した値とその説明を並べる凡例。
    pub fn render_value_legend(&self, values: &[(&str, &str)]) -> Option<String> {
        if self.catalog.locale().is_source() || values.is_empty() {
            return None;
        }
        let mut seen: Vec<(&str, &str)> = values.to_vec();
        seen.sort();
        seen.dedup();

        let mut out = String::new();
        out.push_str(&self.text("legend-heading"));
        out.push('\n');
        for (value, legend) in seen {
            out.push_str(&format!("  {value}: {}\n", self.text(legend)));
        }
        Some(out)
    }

    /// warningをstderrへ出す。
    pub fn print_warning(&self, message: &Msg, stderr: &mut dyn Write) {
        let _ = writeln!(stderr, "{}", self.format(message));
    }

    /// errorをstderrへ出す。
    ///
    /// 翻訳しない安定した英語error ID、選択言語による説明、対処方法、必要な場合は
    /// 外部stderrの原文を、それぞれ別のblockとして表示する。
    pub fn print_error(&self, error: &Error, stderr: &mut dyn Write) {
        for diagnostic in error.diagnostics() {
            self.print_diagnostic(diagnostic, stderr);
        }
    }

    fn print_diagnostic(&self, diagnostic: &Diagnostic, stderr: &mut dyn Write) {
        let _ = writeln!(stderr, "error: {}", diagnostic.id);
        let _ = writeln!(stderr, "{}", self.format(&diagnostic.description));
        if let Some(remediation) = &diagnostic.remediation {
            let _ = writeln!(stderr, "{}", self.format(remediation));
        }
        if let Some(external) = &diagnostic.external {
            // 失敗した工程を同じ形で再実行できるよう、起動そのものを示す。
            let _ = writeln!(
                stderr,
                "{}",
                self.format(&crate::msg!(
                    "external-invocation",
                    program = external.program,
                    args = external.safe_args.join(" ")
                ))
            );
            if let Some(directory) = &external.working_dir {
                let _ = writeln!(
                    stderr,
                    "{}",
                    self.format(&crate::msg!(
                        "external-working-directory",
                        path = crate::paths::display(directory)
                    ))
                );
            }
            if external.stderr_lossy {
                let _ = writeln!(
                    stderr,
                    "{}",
                    self.format(&crate::msg!(
                        "warning-external-output-lossy",
                        program = external.program,
                        stream = "stderr"
                    ))
                );
            }
            if !external.stderr.is_empty() {
                // 外部stderrは翻訳せず、localized説明とは別blockで原文のまま出す。
                let _ = writeln!(
                    stderr,
                    "{}",
                    self.format(&crate::msg!(
                        "external-output-heading",
                        program = external.program
                    ))
                );
                let _ = stderr.write_all(&external.stderr);
                if !external.stderr.ends_with(b"\n") {
                    let _ = writeln!(stderr);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "reporter_test.rs"]
mod reporter_test;
