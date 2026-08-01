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
