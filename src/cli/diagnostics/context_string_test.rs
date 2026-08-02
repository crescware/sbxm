use clap::error::ContextKind;

use crate::i18n::{Catalog, Locale};
use crate::testing::cli::argv;
use crate::testing::outcome::{Checked, Refused, Required};

use super::context_string;

/// CLIが使うparserから、libraryのerrorをそのまま取り出す。
fn parse_failure(arguments: &[&str]) -> Checked<clap::Error> {
    let catalog = Catalog::new(Locale::En);
    crate::cli::build_command(&catalog)?
        .try_get_matches_from(argv(arguments))
        .refused_because("these arguments do not parse")
}

#[test]
fn a_context_holding_one_value_is_taken_as_it_is() -> Checked {
    let error = parse_failure(&["ls", "--nope"])?;
    assert_eq!(
        context_string(&error, ContextKind::InvalidArg),
        Some("--nope".to_string())
    );
    Ok(())
}

/// 候補が複数ある文脈は、1行として読める形へ連ねる。
#[test]
fn a_context_holding_several_values_is_joined_into_one_line() -> Checked {
    // `st`はcommand名の前方一致が複数あるため、候補が複数返る。
    let error = parse_failure(&["st"])?;
    let suggested = context_string(&error, ContextKind::SuggestedSubcommand)
        .required_because("a near miss on a command name offers candidates")?;
    assert!(
        suggested.contains("status") && suggested.contains("stop"),
        "every candidate the library named is kept: {suggested}"
    );
    assert!(
        suggested.contains(", "),
        "the candidates are separated from each other: {suggested}"
    );
    Ok(())
}

/// 装飾付きの文脈は、装飾を落として文字だけを取る。
#[test]
fn a_styled_context_keeps_its_text_and_drops_its_styling() -> Checked {
    let error = parse_failure(&["ls", "--nope"])?;
    let usage = context_string(&error, ContextKind::Usage)
        .required_because("a parse failure carries the usage of the command that failed")?;
    assert!(
        usage.starts_with("Usage: sbxm ls"),
        "the usage line is readable as text: {usage}"
    );
    assert!(
        !usage.contains('\u{1b}'),
        "no terminal escape survives into the diagnostic: {usage:?}"
    );
    Ok(())
}

/// 装飾付きの候補が並ぶ文脈も、同じ規則で1行にする。
#[test]
fn a_context_holding_several_styled_values_is_joined_into_one_line() -> Checked {
    // `--`の後ろはcommand名として読まれない。libraryはその助言を装飾付きで返す。
    let error = parse_failure(&["--", "ls"])?;
    let suggested = context_string(&error, ContextKind::Suggested)
        .required_because("the library explains why the name after -- was not a command")?;
    assert!(
        suggested.contains("subcommand 'ls' exists"),
        "the advice is kept verbatim: {suggested}"
    );
    assert!(
        !suggested.contains('\u{1b}'),
        "no terminal escape survives into the diagnostic: {suggested:?}"
    );
    Ok(())
}

#[test]
fn a_context_the_failure_does_not_carry_is_absent_rather_than_empty() -> Checked {
    let error = parse_failure(&["ls", "--nope"])?;
    assert_eq!(context_string(&error, ContextKind::PriorArg), None);
    assert_eq!(context_string(&error, ContextKind::InvalidValue), None);
    Ok(())
}
