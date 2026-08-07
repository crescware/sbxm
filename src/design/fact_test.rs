use crate::msg;
use crate::testing::outcome::{Checked, Unmet};

use super::*;

fn cause(value: &str) -> Fact {
    Fact::new(msg!("diagnostic-cause-label"), Inline::text(value))
}

/// 1行に収まった値。収まらなかった場合はtestの前提が崩れている。
fn one_line(fact: &Fact) -> Checked<&Inline> {
    match fact {
        Fact::OneLine { value, .. } => Ok(value),
        other => Err(Unmet::new(format!(
            "one raw row was required, but got {other:?}"
        ))),
    }
}

/// 字下げblockになった値。
fn many_lines(fact: &Fact) -> Checked<&[String]> {
    match fact {
        Fact::ManyLines { lines, .. } => Ok(lines),
        other => Err(Unmet::new(format!(
            "an indented block was required, but got {other:?}"
        ))),
    }
}

#[test]
fn a_value_that_fits_one_line_stays_beside_its_label() -> Checked {
    let fact = cause("no digest was reported");
    assert_eq!(one_line(&fact)?.as_str(), "no digest was reported");
    Ok(())
}

#[test]
fn a_trailing_newline_is_not_a_second_line() -> Checked {
    // 外部commandの出力は末尾の改行を伴う。それだけで形を変えない。
    let fact = cause("only one\n");
    assert_eq!(one_line(&fact)?.as_str(), "only one");
    Ok(())
}

#[test]
fn a_value_that_spans_lines_keeps_every_line() -> Checked {
    // 1行へ潰すと、外部が示した位置が失われる。
    let fact = cause("first\nsecond\n");
    assert_eq!(
        many_lines(&fact)?,
        ["first".to_string(), "second".to_string()]
    );
    Ok(())
}

#[test]
fn an_empty_value_is_still_one_row() -> Checked {
    let fact = cause("");
    assert_eq!(one_line(&fact)?.as_str(), "");
    Ok(())
}

#[test]
fn trimming_the_value_does_not_change_what_the_value_means() -> Checked {
    // 値を整えたあとも「これは何か」は変わらない。装飾は`Inline`のvariantが決める。
    let fact = Fact::new(
        msg!("diagnostic-command-label"),
        Inline::important("sbx ls\n"),
    );
    assert_eq!(one_line(&fact)?, &Inline::important("sbx ls"));
    Ok(())
}

#[test]
fn paths_joins_every_entry_as_its_own_line() -> Checked {
    let single = Fact::paths(&["one.txt".to_string()]);
    assert_eq!(single.label(), &msg!("diagnostic-paths-label"));
    assert_eq!(one_line(&single)?.as_str(), "one.txt");

    let many = Fact::paths(&["one.txt".to_string(), "two.txt".to_string()]);
    assert_eq!(
        many_lines(&many)?,
        ["one.txt".to_string(), "two.txt".to_string()]
    );
    Ok(())
}

#[test]
fn every_shape_answers_with_the_label_it_was_given() {
    assert_eq!(cause("one").label(), &msg!("diagnostic-cause-label"));
    assert_eq!(cause("one\ntwo").label(), &msg!("diagnostic-cause-label"));
}

#[test]
fn a_reason_sbxm_observed_itself_stays_a_message() -> Checked {
    // 英語の原文を値へ流し込むと、翻訳された文のなかに英語が残る。
    let fact = Fact::reason(msg!("cause-not-a-regular-file"));
    match fact {
        Fact::Translated { label, value } => {
            assert_eq!(label, msg!("diagnostic-cause-label"));
            assert_eq!(value, msg!("cause-not-a-regular-file"));
            Ok(())
        }
        other => Err(Unmet::new(format!(
            "a translated row was required: {other:?}"
        ))),
    }
}

/// 事実の行に使ってよい項目名と、その値が持つ装飾。
///
/// 語彙は閉じている。同じ意味の行が呼び出し側ごとに違う項目名や違う装飾で出ると、
/// 読み手は毎回どれが照合の基準かを判断し直すことになる。
#[test]
fn every_row_a_diagnostic_can_show_names_itself_the_same_way() -> Checked {
    let vocabulary = [
        (Fact::command("sbx ls"), "diagnostic-command-label", true),
        (
            Fact::directory("/work"),
            "diagnostic-directory-label",
            false,
        ),
        (Fact::path("/work/a.yaml"), "diagnostic-path-label", false),
        (Fact::field("language"), "diagnostic-field-label", true),
        (Fact::entry("manifest.json"), "diagnostic-entry-label", true),
        (
            Fact::source("/host/.gitconfig"),
            "diagnostic-source-label",
            false,
        ),
        (
            Fact::destination(".gitconfig"),
            "diagnostic-destination-label",
            false,
        ),
        (Fact::value("-delete"), "diagnostic-value-label", true),
        (Fact::image("sbxm-example"), "diagnostic-image-label", true),
        (
            Fact::template("sbxm-example"),
            "diagnostic-template-label",
            true,
        ),
        (
            Fact::sandbox("sbxm-example"),
            "diagnostic-sandbox-label",
            true,
        ),
        (
            Fact::document("metadata"),
            "diagnostic-document-label",
            true,
        ),
    ];

    for (fact, label, emphasized) in vocabulary {
        assert_eq!(fact.label(), &msg!(label));
        let value = one_line(&fact)?;
        assert_eq!(
            matches!(value, Inline::Important(_)),
            emphasized,
            "{label} decides its own emphasis: {value:?}"
        );
    }
    Ok(())
}

#[test]
fn the_two_kinds_of_cause_share_one_label() -> Checked {
    // 外部が書いた原文とsbxm自身の観測は、値の出どころだけが違う。
    let observed = Fact::cause("No such file or directory (os error 2)");
    assert_eq!(observed.label(), &msg!("diagnostic-cause-label"));
    assert_eq!(
        one_line(&observed)?,
        &Inline::text("No such file or directory (os error 2)")
    );

    let stated = Fact::reason(msg!("cause-not-a-directory"));
    assert_eq!(stated.label(), &msg!("diagnostic-cause-label"));
    Ok(())
}
