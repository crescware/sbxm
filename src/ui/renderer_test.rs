use super::*;

use crate::error::{Diagnostic, ErrorId, ExternalFailure};
use crate::i18n::Locale;
use crate::ui::diagnostic::Remediation;
use crate::ui::document::{Field, GuidanceItem, LegendEntry};
use crate::ui::policy::CharacterSet;
use crate::ui::style::VisualState;
use crate::ui::table::Cell;

/// ANSI escapeの始まり。色を出さないstreamには1 byteも現れてはならない。
const ESC: u8 = 0x1b;

fn draw(document: &Document, policy: StreamPolicy) -> String {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, policy);
        renderer.write(&Catalog::new(Locale::En), document);
    }
    String::from_utf8(buffer).expect("the renderer writes UTF-8")
}

fn plain(document: &Document) -> String {
    draw(document, StreamPolicy::plain())
}

fn colored(document: &Document) -> String {
    draw(document, StreamPolicy::colored())
}

fn listing() -> Table {
    Table::new(vec![msg!("column-project"), msg!("column-state")])
        .row(vec![
            Inline::text("owner/alpha").into(),
            Inline::state("running", VisualState::Positive).into(),
        ])
        .row(vec![
            Inline::text("owner/bravo").into(),
            Inline::state("stopped", VisualState::Attention).into(),
        ])
}

/// 代表的なblockを一通り含むdocument。matrix testの対象にする。
fn representative() -> Document {
    Document::new()
        .summary(msg!("add-registered", project = "owner/alpha"))
        .fields(
            Some(msg!("status-project-section")),
            vec![Field::new(
                msg!("add-field-project"),
                Inline::important("owner/alpha"),
            )],
        )
        .table(Some(msg!("ls-projects-section")), listing())
        .legend(
            msg!("legend-heading"),
            vec![LegendEntry::new("running", msg!("legend-sandbox-running"))],
        )
        .guidance(
            Some(msg!("add-next-heading")),
            vec![GuidanceItem::Ordered {
                number: 1,
                text: msg!("add-next-prepare"),
            }],
        )
        .try_command("sbxm prepare owner/alpha")
        .note(msg!("files-secret-hint"))
}

#[test]
fn a_stream_without_color_contains_no_escape_byte() {
    let drawn = plain(&representative());
    assert!(
        !drawn.as_bytes().contains(&ESC),
        "a redirected stream must stay copy-and-pasteable: {drawn:?}"
    );
}

#[test]
fn the_standard_theme_never_reaches_for_truecolor_or_256_colors() {
    let drawn = colored(&representative());
    assert!(!drawn.contains("\u{1b}[38;2;"), "{drawn:?}");
    assert!(!drawn.contains("\u{1b}[38;5;"), "{drawn:?}");
    assert!(
        !drawn.contains("\u{1b}[48;"),
        "a background color: {drawn:?}"
    );
}

#[test]
fn the_renderer_never_emits_italic_or_blink() {
    let drawn = colored(&representative());
    assert!(!drawn.contains("\u{1b}[3m"), "italic: {drawn:?}");
    assert!(!drawn.contains("\u{1b}[5m"), "blink: {drawn:?}");
}

#[test]
fn output_never_begins_with_a_blank_line() {
    assert!(!plain(&representative()).starts_with('\n'));
}

#[test]
fn blocks_are_separated_by_exactly_one_blank_line() {
    let drawn = plain(&representative());
    assert!(!drawn.contains("\n\n\n"), "{drawn:?}");
}

#[test]
fn a_document_closes_with_a_single_newline() {
    let drawn = plain(&Document::new().summary(msg!("add-registered", project = "owner/alpha")));
    assert!(drawn.ends_with('\n'));
    assert!(!drawn.ends_with("\n\n"), "{drawn:?}");
}

#[test]
fn a_heading_sits_directly_above_its_content() {
    let drawn = plain(&Document::new().table(Some(msg!("ls-projects-section")), listing()));
    let mut lines = drawn.lines();
    assert_eq!(lines.next(), Some("PROJECTS"));
    assert!(
        lines.next().is_some_and(|line| line.starts_with("PROJECT")),
        "{drawn:?}"
    );
}

#[test]
fn a_command_line_owns_its_line_with_a_blank_line_on_each_side() {
    let drawn = plain(
        &Document::new()
            .guidance(None, vec![GuidanceItem::Plain(msg!("add-next-prepare"))])
            .try_command("sbxm prepare owner/alpha")
            .note(msg!("files-secret-hint")),
    );
    let lines: Vec<&str> = drawn.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "sbxm prepare owner/alpha")
        .expect("the command occupies a line of its own");
    assert_eq!(lines[index - 1], "", "{drawn:?}");
    assert_eq!(lines[index + 1], "", "{drawn:?}");
}

#[test]
fn a_trailing_command_still_closes_with_a_blank_line() {
    let drawn = plain(&Document::new().try_command("sbxm prepare owner/alpha"));
    assert_eq!(drawn, "sbxm prepare owner/alpha\n\n");
}

#[test]
fn a_command_line_carries_nothing_the_user_did_not_type() {
    let drawn = plain(&Document::new().try_command("sbxm prepare owner/alpha"));
    let command = drawn.lines().next().expect("a command line");
    for decoration in ["$", "`", "1.", "- ", "  "] {
        assert!(
            !command.contains(decoration),
            "{decoration:?} is not part of what gets pasted: {command:?}"
        );
    }
}

#[test]
fn consecutive_progress_lines_are_not_separated() {
    let drawn = plain(
        &Document::new()
            .progress(msg!("progress-creating-sandbox"))
            .progress(msg!("progress-starting-sandbox")),
    );
    assert_eq!(drawn.lines().filter(|line| line.is_empty()).count(), 0);
    assert_eq!(drawn.lines().count(), 2);
}

#[test]
fn a_summary_after_progress_is_separated_by_one_blank_line() {
    let drawn = plain(
        &Document::new()
            .progress(msg!("progress-creating-sandbox"))
            .summary(msg!("add-registered", project = "owner/alpha")),
    );
    let lines: Vec<&str> = drawn.lines().collect();
    assert_eq!(lines[1], "", "{drawn:?}");
    assert_eq!(lines.len(), 3, "{drawn:?}");
}

#[test]
fn column_positions_do_not_move_when_color_is_switched_on() {
    let document = Document::new().table(None, listing());
    assert_eq!(plain(&document), strip_ansi(&colored(&document)));
}

#[test]
fn full_width_labels_keep_the_columns_aligned() {
    let document = Document::new().table(
        None,
        Table::new(vec![msg!("column-project"), msg!("column-state")])
            .row(vec![
                Cell::label(msg!("status-item-config")),
                Inline::text("ready").into(),
            ])
            .row(vec![
                Cell::label(msg!("status-item-docker")),
                Inline::text("ready").into(),
            ]),
    );
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, StreamPolicy::plain());
        renderer.write(&Catalog::new(Locale::Ja), &document);
    }
    let drawn = String::from_utf8(buffer).expect("UTF-8");
    let starts: Vec<usize> = drawn
        .lines()
        .skip(1)
        .map(|line| {
            let value = line.rfind("ready").expect("the value column");
            super::display_width(&line[..value])
        })
        .collect();
    assert!(
        starts.windows(2).all(|pair| pair[0] == pair[1]),
        "the value column starts at the same display column: {drawn:?}"
    );
}

#[test]
fn only_the_state_cell_of_a_row_is_colored() {
    let drawn = colored(&Document::new().table(None, listing()));
    let row = drawn
        .lines()
        .find(|line| line.contains("owner/alpha"))
        .expect("the row");
    assert!(
        !row.starts_with('\u{1b}'),
        "the project id stays plain: {row:?}"
    );
    assert!(row.contains("\u{1b}[32mrunning"), "{row:?}");
}

#[test]
fn an_ascii_stream_swaps_the_glyphs_without_changing_the_meaning() {
    let document = Document::new().progress(msg!("progress-creating-sandbox"));
    let unicode = plain(&document);
    let ascii = draw(&document, StreamPolicy::ascii());

    assert!(unicode.starts_with("\u{2192} "), "{unicode:?}");
    assert!(ascii.starts_with("> "), "{ascii:?}");
    assert_eq!(
        unicode.trim_start_matches(['\u{2192}', ' ']),
        ascii.trim_start_matches(['>', ' ']),
        "the sentence is the same either way"
    );
}

#[test]
fn a_warning_names_its_severity_in_words_as_well_as_in_color() {
    let drawn = plain(&Document::new().warning(msg!("destroy-force-notice")));
    assert!(drawn.starts_with("! Warning: "), "{drawn:?}");
}

#[test]
fn a_note_is_told_apart_from_a_warning_without_color() {
    let drawn = plain(&Document::new().note(msg!("files-secret-hint")));
    assert!(drawn.starts_with("! Note: "), "{drawn:?}");
}

#[test]
fn a_diagnostic_keeps_its_id_in_english_behind_a_marker() {
    let drawn = plain(&Document::new().diagnostic(Diagnostic::new(
        ErrorId::DockerUnreachable,
        msg!("error-docker-unreachable", detail = "no answer"),
    )));
    assert!(
        drawn.starts_with("\u{d7} error: docker-unreachable\n"),
        "{drawn:?}"
    );
    // headingと説明は同じblockなので、あいだに空行を置かない。
    assert!(
        drawn
            .lines()
            .nth(1)
            .is_some_and(|line| line.starts_with("  ") && !line.trim().is_empty())
    );
}

#[test]
fn a_remediation_separates_the_explanation_from_the_command() {
    let drawn = plain(
        &Document::new().diagnostic(
            Diagnostic::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = "/x",
                    detail = "no such file"
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-fix-config", path = "/x"))
                    .try_run("sbxm status --global"),
            ),
        ),
    );
    let lines: Vec<&str> = drawn.lines().collect();
    let command = lines
        .iter()
        .position(|line| *line == "sbxm status --global")
        .expect("the command is its own line");
    assert_eq!(lines[command - 1], "", "{drawn:?}");
    assert!(lines.contains(&"  Try:"), "{drawn:?}");
}

fn failed(stderr: &[u8], args: &[&str]) -> Diagnostic {
    Diagnostic::new(
        ErrorId::ExternalCommandFailed,
        msg!(
            "error-external-command-failed",
            program = "docker",
            exit_status = "1"
        ),
    )
    .external(ExternalFailure {
        program: "docker".to_string(),
        safe_args: args.iter().map(|arg| arg.to_string()).collect(),
        working_dir: None,
        exit_status: "1".to_string(),
        stderr: stderr.to_vec(),
        stderr_lossy: false,
    })
}

#[test]
fn external_output_is_indented_and_closed_with_a_newline() {
    // 末尾に改行がない外部outputでも、blockは改行で閉じる。
    let drawn = plain(&Document::new().diagnostic(failed(b"first\nsecond", &["build"])));
    assert!(drawn.contains("\n    first\n    second\n"), "{drawn:?}");
    assert!(drawn.ends_with('\n'));
}

#[test]
fn external_output_is_bracketed_by_a_reset_when_the_stream_is_colored() {
    // 外部が残したstyleを、sbxm自身の出力へ持ち越さない。
    let drawn = colored(&Document::new().diagnostic(failed(b"\x1b[31mred\n", &[])));
    assert!(
        drawn.contains("    \u{1b}[0m\u{1b}[31mred\u{1b}[0m\n"),
        "{drawn:?}"
    );
}

#[test]
fn the_invocation_of_a_failed_command_is_its_own_line() {
    let drawn = plain(&Document::new().diagnostic(failed(b"", &["build", "--tag"])));
    let lines: Vec<&str> = drawn.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "docker build --tag")
        .expect("the invocation is shown as one line");
    assert_eq!(lines[index - 1], "", "{drawn:?}");
}

#[test]
fn several_diagnostics_are_separated_by_one_blank_line() {
    let drawn = plain(
        &Document::new()
            .diagnostic(Diagnostic::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = "/x",
                    detail = "no such file"
                ),
            ))
            .diagnostic(Diagnostic::new(
                ErrorId::DockerUnreachable,
                msg!("error-docker-unreachable", detail = "no answer"),
            )),
    );
    assert!(!drawn.contains("\n\n\n"), "{drawn:?}");
    assert_eq!(drawn.matches("\u{d7} error:").count(), 2, "{drawn:?}");
}

#[test]
fn help_text_keeps_its_own_shape_and_gains_one_trailing_newline() {
    let drawn = plain(&Document::new().verbatim("Usage: sbxm\n\nOptions:\n  --help\n\n\n"));
    assert_eq!(drawn, "Usage: sbxm\n\nOptions:\n  --help\n");
}

#[test]
fn every_matrix_combination_renders_the_same_structure() {
    // localeと文字集合と色を変えても、blockの数と境界の規則は変わらない。
    let document = representative();
    let expected = plain(&document).lines().count();
    for locale in Locale::ALL {
        for policy in [
            StreamPolicy::plain(),
            StreamPolicy::colored(),
            StreamPolicy::ascii(),
        ] {
            let mut buffer: Vec<u8> = Vec::new();
            {
                let mut renderer = Renderer::new(&mut buffer, policy);
                renderer.write(&Catalog::new(locale), &document);
            }
            let drawn = String::from_utf8(buffer).expect("UTF-8");
            assert_eq!(
                drawn.lines().count(),
                expected,
                "{locale} {policy:?} changed the shape: {drawn}"
            );
            assert!(!drawn.starts_with('\n'), "{locale} {policy:?}: {drawn:?}");
            assert!(!drawn.contains("\n\n\n"), "{locale} {policy:?}: {drawn:?}");
            if !policy.color {
                assert!(
                    !drawn.as_bytes().contains(&ESC),
                    "{locale} {policy:?}: {drawn:?}"
                );
            }
        }
    }
}

#[test]
fn a_dumb_terminal_gets_ascii_glyphs_and_no_escape_byte() {
    let policy = StreamPolicy {
        color: false,
        characters: CharacterSet::Ascii,
        width: None,
    };
    let drawn = draw(&representative(), policy);
    assert!(!drawn.as_bytes().contains(&ESC), "{drawn:?}");
    assert!(drawn.starts_with("+ "), "{drawn:?}");
}

/// ANSI sequenceを取り除く。列位置の比較にだけ使う。
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            out.push(character);
            continue;
        }
        for inner in characters.by_ref() {
            if inner.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}
