//! 段落の結合の規則。入力は文字列だけであり、fileもprocessも使わない。

use super::unwrap;

fn joined(text: &str) -> String {
    unwrap(text).joined
}

#[test]
fn english_lines_in_a_paragraph_are_joined_with_a_space() {
    assert_eq!(joined("one\ntwo\nthree\n"), "one two three\n");
}

#[test]
fn japanese_lines_are_joined_without_a_space() {
    assert_eq!(
        joined("日本語の\n文章。\n続く。\n"),
        "日本語の文章。続く。\n"
    );
}

#[test]
fn a_boundary_that_touches_cjk_on_either_side_gets_no_space() {
    assert_eq!(joined("設定は`foo`\nを読む\n"), "設定は`foo`を読む\n");
    assert_eq!(joined("読むのは\n`foo`だ\n"), "読むのは`foo`だ\n");
    assert_eq!(joined("括弧（\n`foo`）\n"), "括弧（`foo`）\n");
}

#[test]
fn two_ascii_words_inside_japanese_text_are_separated_by_a_space() {
    assert_eq!(
        joined("これはunit\ntestである\n"),
        "これはunit testである\n"
    );
}

#[test]
fn paragraphs_separated_by_blank_lines_stay_separate() {
    assert_eq!(joined("a\nb\n\nc\nd\n"), "a b\n\nc d\n");
    assert_eq!(joined("a\nb\n\n\nc\n"), "a b\n\n\nc\n");
}

#[test]
fn trailing_whitespace_on_joined_lines_is_dropped() {
    assert_eq!(joined("a  \n  b \n"), "a b\n");
}

#[test]
fn a_file_without_a_trailing_newline_keeps_none() {
    assert_eq!(joined("a\nb"), "a b");
    assert_eq!(joined(""), "");
}

#[test]
fn joined_line_numbers_are_reported_one_based() {
    assert_eq!(unwrap("a\nb\n\nc\nd\ne\n").joined_lines, [2, 5, 6]);
    assert!(unwrap("a\n\nb\n").joined_lines.is_empty());
}

#[test]
fn the_result_is_stable_under_a_second_pass() {
    let once = joined("one\ntwo\n\n- item\n  more\n\n日本語\nです\n");
    assert_eq!(joined(&once), once);
}

#[test]
fn code_fences_are_left_alone() {
    let text = "```sh\nsbxm add x \\\n  --detach main\n```\ntext\nmore\n";
    assert_eq!(
        joined(text),
        "```sh\nsbxm add x \\\n  --detach main\n```\ntext more\n"
    );
    let tilde = "~~~\nfirst\nsecond\n~~~\n";
    assert_eq!(joined(tilde), tilde);
}

#[test]
fn a_fence_closes_only_with_the_same_marker_at_least_as_long() {
    let nested = "````\n```\nstill code\n```\nstill code\n````\nafter\nwards\n";
    assert_eq!(
        joined(nested),
        "````\n```\nstill code\n```\nstill code\n````\nafter wards\n"
    );
    let mixed = "~~~\n```\na\nb\n~~~\nx\ny\n";
    assert_eq!(joined(mixed), "~~~\n```\na\nb\n~~~\nx y\n");
}

#[test]
fn a_fence_that_never_closes_runs_to_the_end() {
    let text = "```\na\nb\n";
    assert_eq!(joined(text), text);
}

#[test]
fn an_indented_fence_inside_a_list_item_is_left_alone() {
    let text = "1. step\n   ```sh\n   cmd one\n   cmd two\n   ```\n2. next\n";
    assert_eq!(joined(text), text);
}

#[test]
fn front_matter_is_left_alone() {
    let text = "---\ntitle: x\ndescription: y\n---\n\nbody\nline\n";
    assert_eq!(
        joined(text),
        "---\ntitle: x\ndescription: y\n---\n\nbody line\n"
    );
}

#[test]
fn front_matter_that_never_closes_runs_to_the_end() {
    let text = "---\ntitle: x\nbody\nline\n";
    assert_eq!(joined(text), text);
}

#[test]
fn tables_headings_rules_and_quotes_are_not_joined() {
    let text = "# Title\nIntro\n| a | b |\n| - | - |\n> quote\n> more\n---\nafter\n## Two\n  ### Three\n* * *\n";
    assert_eq!(joined(text), text);
}

#[test]
fn seven_hashes_or_a_missing_space_do_not_make_a_heading() {
    assert_eq!(joined("a\n####### b\n"), "a ####### b\n");
    assert_eq!(joined("a\n#hashtag\n"), "a #hashtag\n");
    assert_eq!(joined("a\n#\n"), "a\n#\n");
}

#[test]
fn a_dash_that_is_not_a_rule_or_an_item_continues_the_paragraph() {
    assert_eq!(joined("a\n--\n"), "a --\n");
    assert_eq!(joined("a\n-x\n"), "a -x\n");
    assert_eq!(joined("a\n    ---\n"), "a ---\n");
}

#[test]
fn html_blocks_are_left_alone_until_a_blank_line() {
    let text =
        "<p align=\"center\">\n  <a href=\"x\">y</a>\n</p>\n\ntext\nmore\n<!-- note\nstill -->\n";
    assert_eq!(
        joined(text),
        "<p align=\"center\">\n  <a href=\"x\">y</a>\n</p>\n\ntext more\n<!-- note\nstill -->\n"
    );
}

#[test]
fn a_fence_inside_an_html_block_still_opens_a_code_block() {
    let text = "<div>\n```\na\nb\n```\n</div>\n";
    assert_eq!(joined(text), text);
}

#[test]
fn list_item_continuations_are_joined_but_nested_items_are_not() {
    let text = "- first item\n  continues here\n  - nested item\n    also continues\n- second\n+ plus\n  more\n";
    assert_eq!(
        joined(text),
        "- first item continues here\n  - nested item also continues\n- second\n+ plus more\n"
    );
}

#[test]
fn ordered_items_take_lazy_continuations() {
    assert_eq!(
        joined("1. one\ncontinued\n2. two\n10) ten\n"),
        "1. one continued\n2. two\n10) ten\n"
    );
}

#[test]
fn a_number_without_a_marker_is_text() {
    assert_eq!(
        joined("version\n2.36 or later\n"),
        "version 2.36 or later\n"
    );
    assert_eq!(joined("a\n1.\n"), "a 1.\n");
}

#[test]
fn an_indented_code_block_outside_a_paragraph_is_left_alone() {
    let text = "text\n\n    code line\n    more code\n";
    assert_eq!(joined(text), text);
}

#[test]
fn an_indented_line_inside_a_paragraph_continues_it() {
    assert_eq!(joined("text\n    continued\n"), "text continued\n");
    assert_eq!(joined("- item\n    continued\n"), "- item continued\n");
}
