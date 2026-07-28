//! Workflowと、利用者向け出力の共通部分。
//!
//! stdoutは正常結果に使用する。状態値は翻訳しないため、正本locale以外ではtableへ
//! 状態値の凡例を加える。stderrはprompt、warning、errorに使用する。

pub mod add;
pub mod daemon;
pub mod destroy;
pub mod files;
pub mod host_clone;
pub mod identity;
pub mod image;
pub mod init;
pub mod inventory;
pub mod ls;
pub mod open;
pub mod prepare;
pub mod protection;
pub mod rebuild;
pub mod repository;
pub mod sandbox;
pub mod secret;
pub mod select;
pub mod status_global;
pub mod status_project;
pub mod stop;
pub mod sync_files;
pub mod template;
pub mod worktree;

use std::io::Write;

use crate::error::{Diagnostic, Error, Msg};
use crate::i18n::Catalog;

/// 表示に使う状態値。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatusValue {
    Ready,
    Missing,
    Error,
    Running,
    Stopped,
}

impl StatusValue {
    pub fn as_str(self) -> &'static str {
        match self {
            StatusValue::Ready => "ready",
            StatusValue::Missing => "missing",
            StatusValue::Error => "error",
            StatusValue::Running => "running",
            StatusValue::Stopped => "stopped",
        }
    }

    /// 凡例に使うFTL message ID。
    fn legend_id(self) -> &'static str {
        match self {
            StatusValue::Ready => "legend-ready",
            StatusValue::Missing => "legend-missing",
            StatusValue::Error => "legend-error",
            StatusValue::Running => "legend-running",
            StatusValue::Stopped => "legend-stopped",
        }
    }
}

impl std::fmt::Display for StatusValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 1行分の診断結果。
#[derive(Debug, Clone)]
pub struct Row {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub status: StatusValue,
}

/// 全角文字を2桁として数える表示幅。
///
/// 日本語modeでも列を揃えるために使う。日本語modeのstdoutは機械可読な出力契約としない。
pub fn display_width(text: &str) -> usize {
    text.chars()
        .map(|character| if is_wide(character) { 2 } else { 1 })
        .sum()
}

fn is_wide(character: char) -> bool {
    matches!(character as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD)
}

/// 1行分を、列幅にそろえて描画する。末尾の余白は残さない。
fn render_row(values: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, value) in values.iter().enumerate() {
        if index + 1 == values.len() {
            line.push_str(value);
        } else {
            line.push_str(&pad_to(value, widths[index]));
        }
    }
    line.push('\n');
    line
}

fn pad_to(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    for _ in display_width(text)..width {
        out.push(' ');
    }
    out
}

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
mod tests {
    use super::*;
    use crate::error::{ErrorId, ExternalFailure};
    use crate::i18n::Locale;
    use crate::msg;

    fn rows() -> Vec<Row> {
        vec![
            Row {
                item: "status-item-config",
                status: StatusValue::Ready,
            },
            Row {
                item: "status-item-daemon",
                status: StatusValue::Error,
            },
        ]
    }

    #[test]
    fn wide_characters_count_as_two_columns() {
        assert_eq!(display_width("Config"), 6);
        assert_eq!(display_width("設定 (Config)"), 4 + 9);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn the_english_table_keeps_the_published_column_names() {
        let catalog = Catalog::new(Locale::En);
        let reporter = Reporter::new(&catalog);
        let table = reporter.render_status_table(
            "status-global-section",
            "status-column-item",
            "status-column-status",
            &rows(),
        );
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[0], "GLOBAL");
        assert!(lines[1].starts_with("ITEM"), "{}", lines[1]);
        assert!(lines[1].ends_with("STATUS"), "{}", lines[1]);
        assert!(lines[2].starts_with("Config"), "{}", lines[2]);
        assert!(lines[2].ends_with("ready"), "{}", lines[2]);
        assert!(lines[3].ends_with("error"), "{}", lines[3]);
    }

    #[test]
    fn columns_line_up_in_both_languages() {
        for locale in Locale::ALL {
            let catalog = Catalog::new(locale);
            let reporter = Reporter::new(&catalog);
            let table = reporter.render_status_table(
                "status-global-section",
                "status-column-item",
                "status-column-status",
                &rows(),
            );
            let lines: Vec<&str> = table.lines().collect();
            let status_header = catalog.text("status-column-status").unwrap();

            let mut offsets = vec![display_width(
                lines[1]
                    .strip_suffix(&status_header)
                    .expect("the header line ends with the status column name"),
            )];
            for (row, line) in rows().iter().zip(lines.iter().skip(2)) {
                offsets.push(display_width(
                    line.strip_suffix(row.status.as_str())
                        .expect("each row ends with its status value"),
                ));
            }
            assert!(
                offsets.windows(2).all(|pair| pair[0] == pair[1]),
                "{locale}: the status column must start at one fixed offset: {offsets:?}"
            );
        }
    }

    #[test]
    fn status_values_are_never_translated() {
        for locale in Locale::ALL {
            let catalog = Catalog::new(locale);
            let reporter = Reporter::new(&catalog);
            let table = reporter.render_status_table(
                "status-global-section",
                "status-column-item",
                "status-column-status",
                &rows(),
            );
            assert!(table.contains("ready"), "{locale}: {table}");
            assert!(table.contains("error"), "{locale}: {table}");
        }
    }

    #[test]
    fn the_legend_lists_only_the_values_that_actually_appeared() {
        for locale in Locale::ALL.into_iter().filter(|locale| !locale.is_source()) {
            let catalog = Catalog::new(locale);
            let reporter = Reporter::new(&catalog);
            let legend = reporter.render_legend(&rows()).unwrap_or_else(|| {
                panic!("{locale} is not the source locale, so it adds a legend")
            });
            assert!(legend.contains("ready:"), "{locale}: {legend}");
            assert!(legend.contains("error:"), "{locale}: {legend}");
            assert!(
                !legend.contains("stopped:"),
                "{locale}: values that did not appear must be left out: {legend}"
            );
        }
    }

    #[test]
    fn the_source_locale_has_no_legend() {
        // 状態値は正本localeの語であるため、正本localeでは注釈を出さない。
        let catalog = Catalog::new(Locale::SOURCE);
        let reporter = Reporter::new(&catalog);
        assert!(reporter.render_legend(&rows()).is_none());
    }

    #[test]
    fn errors_show_the_stable_id_the_description_and_the_remediation() {
        let catalog = Catalog::new(Locale::En);
        let reporter = Reporter::new(&catalog);
        let error = Error::single(
            Diagnostic::new(
                ErrorId::ConfigMissing,
                msg!(
                    "error-config-missing",
                    path = "/home/example/.sbxm/config.toml"
                ),
            )
            .remediation(msg!("remediation-run-init")),
        );

        let mut buffer = Vec::new();
        reporter.print_error(&error, &mut buffer);
        let text = String::from_utf8(buffer).unwrap();

        assert!(text.starts_with("error: config-missing\n"), "{text}");
        assert!(text.contains("/home/example/.sbxm/config.toml"), "{text}");
        assert!(text.contains("sbxm init"), "{text}");
    }

    #[test]
    fn external_stderr_is_shown_verbatim_in_its_own_block() {
        let catalog = Catalog::new(Locale::Ja);
        let reporter = Reporter::new(&catalog);
        let error = Error::single(
            Diagnostic::new(
                ErrorId::ExternalCommandFailed,
                msg!(
                    "error-external-command-failed",
                    program = "sbx",
                    exit_status = "exit status: 2"
                ),
            )
            .external(ExternalFailure {
                program: "sbx".into(),
                safe_args: vec!["ls".into()],
                working_dir: None,
                exit_status: "exit status: 2".into(),
                stderr: b"Error: daemon is not running".to_vec(),
                stderr_lossy: false,
            }),
        );

        let mut buffer = Vec::new();
        reporter.print_error(&error, &mut buffer);
        let text = String::from_utf8(buffer).unwrap();

        assert!(text.contains("error: external-command-failed"), "{text}");
        assert!(
            text.contains("Error: daemon is not running"),
            "the external message must be preserved verbatim: {text}"
        );
        assert!(text.ends_with('\n'), "{text:?}");
    }

    #[test]
    fn a_failed_command_is_shown_with_the_invocation_that_produced_it() {
        let catalog = Catalog::new(Locale::En);
        let reporter = Reporter::new(&catalog);
        let error = Error::single(
            Diagnostic::new(
                ErrorId::ExternalCommandFailed,
                msg!(
                    "error-external-command-failed",
                    program = "git",
                    exit_status = "exit status: 128"
                ),
            )
            .external(ExternalFailure {
                program: "git".into(),
                safe_args: vec!["clone".into(), "--bare".into()],
                working_dir: Some(std::path::PathBuf::from("/Users/example/Projects")),
                exit_status: "exit status: 128".into(),
                stderr: Vec::new(),
                stderr_lossy: false,
            }),
        );

        let mut buffer = Vec::new();
        reporter.print_error(&error, &mut buffer);
        let text = String::from_utf8(buffer).unwrap();

        assert!(text.contains("git clone --bare"), "{text}");
        assert!(text.contains("/Users/example/Projects"), "{text}");
    }

    #[test]
    fn a_lossy_external_stream_is_reported_as_such() {
        let catalog = Catalog::new(Locale::En);
        let reporter = Reporter::new(&catalog);
        let error = Error::single(
            Diagnostic::new(
                ErrorId::ExternalCommandFailed,
                msg!(
                    "error-external-command-failed",
                    program = "sbx",
                    exit_status = "exit status: 1"
                ),
            )
            .external(ExternalFailure {
                program: "sbx".into(),
                safe_args: Vec::new(),
                working_dir: None,
                exit_status: "exit status: 1".into(),
                stderr: vec![0xff, b'a'],
                stderr_lossy: true,
            }),
        );

        let mut buffer = Vec::new();
        reporter.print_error(&error, &mut buffer);
        let text = String::from_utf8_lossy(&buffer);
        assert!(text.contains("not valid UTF-8"), "{text}");
    }

    #[test]
    fn every_diagnostic_of_a_multi_error_run_is_shown() {
        let catalog = Catalog::new(Locale::En);
        let reporter = Reporter::new(&catalog);
        let error = Error::many(vec![
            Diagnostic::new(
                ErrorId::ConfigMissing,
                msg!("error-config-missing", path = "/a"),
            ),
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", command = "sbx"),
            ),
        ]);

        let mut buffer = Vec::new();
        reporter.print_error(&error, &mut buffer);
        let text = String::from_utf8(buffer).unwrap();
        assert!(text.contains("error: config-missing"), "{text}");
        assert!(text.contains("error: host-command-missing"), "{text}");
    }
}
