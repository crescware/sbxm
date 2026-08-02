use crate::design::document::Document;
use crate::design::policy::StreamPolicy;
use crate::design::table::Table;
use crate::design::text::Inline;
use crate::i18n::Catalog;
use crate::msg;

use crate::testing::outcome::{Checked, Required};

use super::*;

use crate::design::Cell;
use crate::design::diagnostic::Remediation;
use crate::design::policy::CharacterSet;
use crate::design::style::VisualState;
use crate::design::{Fact, Field, GuidanceItem, LegendEntry};
use crate::diagnostics::{Diagnostic, ErrorId, ExternalFailure};
use crate::i18n::Locale;

/// ANSI escapeの始まり。色を出さないstreamには1 byteも現れてはならない。
const ESC: u8 = 0x1b;

fn draw(document: &Document, policy: StreamPolicy) -> Checked<String> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, policy);
        renderer.write(&Catalog::new(Locale::En), document);
    }
    String::from_utf8(buffer).required_because("the renderer writes UTF-8")
}

fn plain(document: &Document) -> Checked<String> {
    draw(document, StreamPolicy::plain())
}

fn colored(document: &Document) -> Checked<String> {
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
fn a_stream_without_color_contains_no_escape_byte() -> Checked {
    let drawn = plain(&representative())?;
    assert!(
        !drawn.as_bytes().contains(&ESC),
        "a redirected stream must stay copy-and-pasteable: {drawn:?}"
    );
    Ok(())
}

#[test]
fn the_standard_theme_never_reaches_for_truecolor_or_256_colors() -> Checked {
    let drawn = colored(&representative())?;
    assert!(!drawn.contains("\u{1b}[38;2;"), "{drawn:?}");
    assert!(!drawn.contains("\u{1b}[38;5;"), "{drawn:?}");
    assert!(
        !drawn.contains("\u{1b}[48;"),
        "a background color: {drawn:?}"
    );
    Ok(())
}

#[test]
fn the_renderer_never_emits_italic_or_blink() -> Checked {
    let drawn = colored(&representative())?;
    assert!(!drawn.contains("\u{1b}[3m"), "italic: {drawn:?}");
    assert!(!drawn.contains("\u{1b}[5m"), "blink: {drawn:?}");
    Ok(())
}

#[test]
fn output_never_begins_with_a_blank_line() -> Checked {
    assert!(!plain(&representative())?.starts_with('\n'));
    Ok(())
}

#[test]
fn blocks_are_separated_by_exactly_one_blank_line() -> Checked {
    let drawn = plain(&representative())?;
    assert!(!drawn.contains("\n\n\n"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_document_closes_with_a_single_newline() -> Checked {
    let drawn = plain(&Document::new().summary(msg!("add-registered", project = "owner/alpha")))?;
    assert!(drawn.ends_with('\n'));
    assert!(!drawn.ends_with("\n\n"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_heading_sits_directly_above_its_content() -> Checked {
    let drawn = plain(&Document::new().table(Some(msg!("ls-projects-section")), listing()))?;
    let mut lines = drawn.lines();
    assert_eq!(lines.next(), Some("PROJECTS"));
    assert!(
        lines.next().is_some_and(|line| line.starts_with("PROJECT")),
        "{drawn:?}"
    );
    Ok(())
}

#[test]
fn a_command_line_owns_its_line_with_a_blank_line_on_each_side() -> Checked {
    let drawn = plain(
        &Document::new()
            .guidance(None, vec![GuidanceItem::Plain(msg!("add-next-prepare"))])
            .try_command("sbxm prepare owner/alpha")
            .note(msg!("files-secret-hint")),
    )?;
    let lines: Vec<&str> = drawn.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "sbxm prepare owner/alpha")
        .required_because("the command occupies a line of its own")?;
    assert_eq!(lines[index - 1], "", "{drawn:?}");
    assert_eq!(lines[index + 1], "", "{drawn:?}");
    Ok(())
}

#[test]
fn a_trailing_command_still_closes_with_a_blank_line() -> Checked {
    let drawn = plain(&Document::new().try_command("sbxm prepare owner/alpha"))?;
    assert_eq!(drawn, "sbxm prepare owner/alpha\n\n");
    Ok(())
}

#[test]
fn a_command_line_carries_nothing_the_user_did_not_type() -> Checked {
    let drawn = plain(&Document::new().try_command("sbxm prepare owner/alpha"))?;
    let command = drawn.lines().next().required_because("a command line")?;
    for decoration in ["$", "`", "1.", "- ", "  "] {
        assert!(
            !command.contains(decoration),
            "{decoration:?} is not part of what gets pasted: {command:?}"
        );
    }
    Ok(())
}

#[test]
fn consecutive_progress_lines_are_not_separated() -> Checked {
    let drawn = plain(
        &Document::new()
            .progress(msg!("progress-creating-sandbox"))
            .progress(msg!("progress-starting-sandbox")),
    )?;
    assert_eq!(drawn.lines().filter(|line| line.is_empty()).count(), 0);
    assert_eq!(drawn.lines().count(), 2);
    Ok(())
}

#[test]
fn a_summary_after_progress_is_separated_by_one_blank_line() -> Checked {
    let drawn = plain(
        &Document::new()
            .progress(msg!("progress-creating-sandbox"))
            .summary(msg!("add-registered", project = "owner/alpha")),
    )?;
    let lines: Vec<&str> = drawn.lines().collect();
    assert_eq!(lines[1], "", "{drawn:?}");
    assert_eq!(lines.len(), 3, "{drawn:?}");
    Ok(())
}

#[test]
fn column_positions_do_not_move_when_color_is_switched_on() -> Checked {
    let document = Document::new().table(None, listing());
    assert_eq!(plain(&document)?, strip_ansi(&colored(&document)?));
    Ok(())
}

#[test]
fn full_width_labels_keep_the_columns_aligned() -> Checked {
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
    let drawn = String::from_utf8(buffer).required_because("UTF-8")?;
    let mut starts: Vec<usize> = Vec::new();
    for line in drawn.lines().skip(1) {
        let value = line.rfind("ready").required_because("the value column")?;
        starts.push(crate::design::width::display_width(&line[..value]));
    }
    assert!(
        starts.windows(2).all(|pair| pair[0] == pair[1]),
        "the value column starts at the same display column: {drawn:?}"
    );
    Ok(())
}

#[test]
fn only_the_state_cell_of_a_row_is_colored() -> Checked {
    let drawn = colored(&Document::new().table(None, listing()))?;
    let row = drawn
        .lines()
        .find(|line| line.contains("owner/alpha"))
        .required_because("the row")?;
    assert!(
        !row.starts_with('\u{1b}'),
        "the project id stays plain: {row:?}"
    );
    assert!(row.contains("\u{1b}[32mrunning"), "{row:?}");
    Ok(())
}

#[test]
fn an_ascii_stream_swaps_the_glyphs_without_changing_the_meaning() -> Checked {
    let document = Document::new().progress(msg!("progress-creating-sandbox"));
    let unicode = plain(&document)?;
    let ascii = draw(&document, StreamPolicy::ascii())?;

    assert!(unicode.starts_with("\u{2192} "), "{unicode:?}");
    assert!(ascii.starts_with("> "), "{ascii:?}");
    assert_eq!(
        unicode.trim_start_matches(['\u{2192}', ' ']),
        ascii.trim_start_matches(['>', ' ']),
        "the sentence is the same either way"
    );
    Ok(())
}

#[test]
fn a_warning_names_its_severity_in_words_as_well_as_in_color() -> Checked {
    let drawn = plain(&Document::new().warning(msg!("destroy-force-notice"), Vec::new()))?;
    assert!(drawn.starts_with("! Warning: "), "{drawn:?}");
    Ok(())
}

#[test]
fn a_note_is_told_apart_from_a_warning_without_color() -> Checked {
    let drawn = plain(&Document::new().note(msg!("files-secret-hint")))?;
    assert!(drawn.starts_with("! Note: "), "{drawn:?}");
    Ok(())
}

#[test]
fn a_diagnostic_keeps_its_id_in_english_behind_a_marker() -> Checked {
    let drawn = plain(&Document::new().diagnostic(Diagnostic::new(
        ErrorId::DockerUnreachable,
        msg!("error-docker-unreachable"),
    )))?;
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
    Ok(())
}

#[test]
fn a_remediation_separates_the_explanation_from_the_command() -> Checked {
    let drawn = plain(
        &Document::new().diagnostic(
            Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                .fact(Fact::path("/x"))
                .remediation(
                    Remediation::text(msg!("remediation-fix-config", path = "/x"))
                        .try_run("sbxm status --global"),
                ),
        ),
    )?;
    let lines: Vec<&str> = drawn.lines().collect();
    let command = lines
        .iter()
        .position(|line| *line == "sbxm status --global")
        .required_because("the command is its own line")?;
    assert_eq!(lines[command - 1], "", "{drawn:?}");
    assert!(lines.contains(&"  Try:"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_remediation_that_is_only_a_command_gets_no_heading_above_it() -> Checked {
    // `Try:`は説明のための見出しである。説明が無いのに見出しだけを出すと、読み手は
    // 抜け落ちた文を探すことになる。
    let drawn = plain(
        &Document::new().diagnostic(
            Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                .remediation(Remediation::new().try_run("sbxm status --global")),
        ),
    )?;
    assert!(!drawn.contains("Try:"), "{drawn:?}");

    // 見出しが無くなっても、commandは空行で挟んだ独立blockのままである。
    let lines: Vec<&str> = drawn.lines().collect();
    let command = lines
        .iter()
        .position(|line| *line == "sbxm status --global")
        .required_because("the command is its own line")?;
    assert_eq!(lines[command - 1], "", "{drawn:?}");
    assert!(drawn.ends_with("sbxm status --global\n\n"), "{drawn:?}");
    Ok(())
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
        safe_args: args.iter().map(|arg| (*arg).to_string()).collect(),
        working_dir: None,
        exit_status: "1".to_string(),
        stderr: stderr.to_vec(),
        stderr_lossy: false,
    })
}

#[test]
fn external_output_is_indented_and_closed_with_a_newline() -> Checked {
    // 末尾に改行がない外部outputでも、blockは改行で閉じる。
    let drawn = plain(&Document::new().diagnostic(failed(b"first\nsecond", &["build"])))?;
    assert!(drawn.contains("\n    first\n    second\n"), "{drawn:?}");
    assert!(drawn.ends_with('\n'));
    Ok(())
}

#[test]
fn external_output_is_bracketed_by_a_reset_when_the_stream_is_colored() -> Checked {
    // 外部が残したstyleを、sbxm自身の出力へ持ち越さない。
    let drawn = colored(&Document::new().diagnostic(failed(b"\x1b[31mred\n", &[])))?;
    assert!(
        drawn.contains("    \u{1b}[0m\u{1b}[31mred\u{1b}[0m\n"),
        "{drawn:?}"
    );
    Ok(())
}

/// 描いたbyte列。原文が有効なUTF-8でない場合も、変換せずに数えられるようにする。
fn drawn_bytes(document: &Document, policy: StreamPolicy) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, policy);
        renderer.write(&Catalog::new(Locale::En), document);
    }
    buffer
}

#[test]
fn output_that_was_not_valid_utf8_says_so_and_still_shows_the_original_bytes() -> Checked {
    // 読めなかったのはsbxmであり、外部が書いたbyteではない。原文は加工せずに出し、
    // 読み取りについての注意を別の行で述べる。
    let mut diagnostic = failed(b"boom \xff\n", &["build"]);
    if let Some(external) = diagnostic.external.as_mut() {
        external.stderr_lossy = true;
    }
    let buffer = drawn_bytes(
        &Document::new().diagnostic(diagnostic),
        StreamPolicy::plain(),
    );
    assert!(
        buffer.windows(10).any(|window| window == b"    boom \xff"),
        "the original bytes are passed through: {buffer:?}"
    );

    let drawn = String::from_utf8_lossy(&buffer).into_owned();
    let lines: Vec<&str> = drawn.lines().collect();
    let notice = lines
        .iter()
        .position(|line| line.starts_with("! Warning: "))
        .required_because("the lossy conversion is reported as a warning")?;
    // どのcommandのどのstreamが読めなかったかを、注意そのものが持つ。
    assert!(lines[notice].contains("stderr"), "{drawn:?}");
    assert!(lines[notice].contains("docker"), "{drawn:?}");
    // 注意は引用の前に置く。読んだあとで読み方を知らされても遅い。
    let heading = lines
        .iter()
        .position(|line| *line == "  Output of docker:")
        .required_because("the quoted output keeps its heading")?;
    assert!(notice < heading, "{drawn:?}");
    Ok(())
}

#[test]
fn a_lossy_read_is_reported_even_when_the_command_wrote_nothing_readable() -> Checked {
    // 何も引用できなくても、読み取りに失敗したことは伝える。
    let mut diagnostic = failed(b"", &["build"]);
    if let Some(external) = diagnostic.external.as_mut() {
        external.stderr_lossy = true;
    }
    let drawn = plain(&Document::new().diagnostic(diagnostic))?;
    assert!(drawn.contains("! Warning: "), "{drawn:?}");
    assert!(!drawn.contains("Output of docker:"), "{drawn:?}");
    // 空のblockが末尾の空行を増やさない。
    assert!(!drawn.ends_with("\n\n"), "{drawn:?}");
    Ok(())
}

#[test]
fn the_invocation_of_a_failed_command_is_a_labelled_row_next_to_the_description() -> Checked {
    // 実行を求めるcommandではないため、独立blockにも空行にも頼らない。
    let drawn = plain(&Document::new().diagnostic(failed(b"", &["build", "--tag"])))?;
    let lines: Vec<&str> = drawn.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "  Command: docker build --tag")
        .required_because("the invocation is one labelled row")?;
    assert_ne!(lines[index - 1], "", "{drawn:?}");
    assert!(!drawn.contains("\ndocker build --tag"), "{drawn:?}");
    Ok(())
}

#[test]
fn the_working_directory_follows_the_command_as_another_row() -> Checked {
    let mut diagnostic = failed(b"", &["build"]);
    if let Some(external) = diagnostic.external.as_mut() {
        external.working_dir = Some(std::path::PathBuf::from("/tmp/example"));
    }
    let drawn = plain(&Document::new().diagnostic(diagnostic))?;
    assert!(
        drawn.contains("  Command: docker build\n  Working directory: /tmp/example\n"),
        "{drawn:?}"
    );
    Ok(())
}

/// `sbx ls`の出力を読めなかったときの診断。事実だけを持ち、外部stderrは持たない。
fn unreadable_output(cause: &str) -> Diagnostic {
    Diagnostic::new(
        ErrorId::ExternalOutputUnparseable,
        msg!("error-external-output-unparseable"),
    )
    .fact(Fact::new(
        msg!("diagnostic-command-label"),
        Inline::important("sbx ls"),
    ))
    .fact(Fact::new(
        msg!("diagnostic-cause-label"),
        Inline::text(cause),
    ))
}

#[test]
fn a_fact_keeps_the_label_and_the_value_on_one_line_below_the_description() -> Checked {
    let drawn = plain(&Document::new().diagnostic(unreadable_output(
        "EOF while parsing an object at line 1 column 1",
    )))?;
    assert_eq!(
        drawn,
        concat!(
            "\u{d7} error: external-output-unparseable\n",
            "  The output could not be interpreted\n",
            "  Command: sbx ls\n",
            "  Cause: EOF while parsing an object at line 1 column 1\n",
        ),
        "{drawn:?}"
    );
    Ok(())
}

#[test]
fn a_fact_that_does_not_fit_one_line_is_indented_under_its_label() -> Checked {
    // 改行を1行へ潰すと、外部が示した位置が失われる。
    let drawn = plain(&Document::new().diagnostic(unreadable_output("first\nsecond\n")))?;
    assert!(
        drawn.contains("  Cause:\n    first\n    second\n"),
        "{drawn:?}"
    );
    assert!(drawn.ends_with("    second\n"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_value_that_only_ends_with_a_newline_stays_on_the_label_line() -> Checked {
    let drawn = plain(&Document::new().diagnostic(unreadable_output("only one\n")))?;
    assert!(drawn.contains("  Cause: only one\n"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_fact_without_a_value_leaves_no_trailing_space() -> Checked {
    let drawn = plain(&Document::new().diagnostic(unreadable_output("")))?;
    assert!(drawn.contains("  Cause\u{3a}\n"), "{drawn:?}");
    assert!(!drawn.contains(" \n"), "{drawn:?}");
    Ok(())
}

#[test]
fn a_fact_label_is_muted_and_the_matching_value_is_the_one_that_stands_out() -> Checked {
    // 色は足さない。dimな項目名とboldな値で境界を作る。
    let drawn = colored(&Document::new().diagnostic(unreadable_output("unreadable")))?;
    let command = drawn
        .lines()
        .find(|line| line.contains("sbx ls"))
        .required_because("the command row is drawn")?;
    assert!(command.starts_with("  \u{1b}[2mCommand:"), "{command:?}");
    assert!(command.contains("\u{1b}[1msbx ls"), "{command:?}");
    // 原因は外部の原文であり、強調も着色もしない。
    let cause = drawn
        .lines()
        .find(|line| line.contains("unreadable"))
        .required_because("the cause row is drawn")?;
    assert!(cause.ends_with("\u{1b}[0m unreadable"), "{cause:?}");
    Ok(())
}

#[test]
fn a_fact_that_spans_lines_is_bracketed_by_a_reset_when_the_stream_is_colored() -> Checked {
    // 外部が残したstyleを、sbxm自身の出力へ持ち越さない。
    let drawn = colored(&Document::new().diagnostic(unreadable_output("\u{1b}[31mred\nplain")))?;
    assert!(
        drawn.contains("    \u{1b}[0m\u{1b}[31mred\u{1b}[0m\n"),
        "{drawn:?}"
    );
    Ok(())
}

#[test]
fn a_diagnostic_with_facts_keeps_its_shape_in_every_locale_and_policy() -> Checked {
    let document = Document::new()
        .diagnostic(unreadable_output("first\nsecond"))
        .diagnostic(failed(b"boom", &["build"]));
    let expected = plain(&document)?.lines().count();
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
            let drawn = String::from_utf8(buffer).required_because("UTF-8")?;
            assert_eq!(
                drawn.lines().count(),
                expected,
                "{locale} {policy:?} changed the shape: {drawn}"
            );
            assert!(!drawn.contains("\n\n\n"), "{locale} {policy:?}: {drawn:?}");
            if !policy.color {
                assert!(
                    !drawn.as_bytes().contains(&ESC),
                    "{locale} {policy:?}: {drawn:?}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn several_diagnostics_are_separated_by_one_blank_line() -> Checked {
    let drawn = plain(
        &Document::new()
            .diagnostic(Diagnostic::new(
                ErrorId::ConfigUnreadable,
                msg!("error-config-unreadable"),
            ))
            .diagnostic(Diagnostic::new(
                ErrorId::DockerUnreachable,
                msg!("error-docker-unreachable"),
            )),
    )?;
    assert!(!drawn.contains("\n\n\n"), "{drawn:?}");
    assert_eq!(drawn.matches("\u{d7} error:").count(), 2, "{drawn:?}");
    Ok(())
}

#[test]
fn help_text_keeps_its_own_shape_and_gains_one_trailing_newline() -> Checked {
    let drawn = plain(&Document::new().verbatim("Usage: sbxm\n\nOptions:\n  --help\n\n\n"))?;
    assert_eq!(drawn, "Usage: sbxm\n\nOptions:\n  --help\n");
    Ok(())
}

#[test]
fn every_matrix_combination_renders_the_same_structure() -> Checked {
    // localeと文字集合と色を変えても、blockの数と境界の規則は変わらない。
    let document = representative();
    let expected = plain(&document)?.lines().count();
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
            let drawn = String::from_utf8(buffer).required_because("UTF-8")?;
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
    Ok(())
}

#[test]
fn a_dumb_terminal_gets_ascii_glyphs_and_no_escape_byte() -> Checked {
    let policy = StreamPolicy {
        color: false,
        characters: CharacterSet::Ascii,
        width: None,
    };
    let drawn = draw(&representative(), policy)?;
    assert!(!drawn.as_bytes().contains(&ESC), "{drawn:?}");
    assert!(drawn.starts_with("+ "), "{drawn:?}");
    Ok(())
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

#[test]
fn a_warning_shows_its_facts_under_the_marker_line() -> Checked {
    // warningは軽い診断ではなく、止まらずに報告する同じ形である。
    let drawn = plain(&Document::new().warning(
        msg!("warning-build-context-left-behind"),
        vec![
            Fact::path("/tmp/sbxm-build-context-a41f"),
            Fact::cause("Directory not empty (os error 39)"),
        ],
    ))?;
    let lines: Vec<&str> = drawn.lines().collect();
    assert!(lines[0].starts_with("! Warning: "), "{drawn:?}");
    assert_eq!(
        lines[1], "  Path: /tmp/sbxm-build-context-a41f",
        "{drawn:?}"
    );
    assert_eq!(
        lines[2], "  Cause: Directory not empty (os error 39)",
        "{drawn:?}"
    );
    Ok(())
}

#[test]
fn a_cause_sbxm_observed_itself_is_drawn_in_the_chosen_language() -> Checked {
    // 外部の原文は訳さないが、sbxm自身の観測は訳す。値の出どころで経路が分かれる。
    let document = Document::new().diagnostic(
        Diagnostic::new(
            ErrorId::MetadataUnreadable,
            msg!("error-metadata-unreadable"),
        )
        .fact(Fact::path("/work/.sbxm/metadata.yaml"))
        .fact(Fact::reason(msg!("cause-symbolic-link"))),
    );
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, StreamPolicy::plain());
        renderer.write(&Catalog::new(Locale::Ja), &document);
    }
    let drawn = String::from_utf8(buffer).required_because("UTF-8")?;
    assert!(
        drawn.contains("  原因: pathがsymbolic linkです"),
        "{drawn:?}"
    );
    assert!(
        drawn.contains("  path: /work/.sbxm/metadata.yaml"),
        "{drawn:?}"
    );
    Ok(())
}
