use crate::design::style::VisualState;

use crate::testing::outcome::{Checked, Required};

use super::*;

#[test]
fn a_command_line_keeps_what_the_user_types() -> Checked {
    let command =
        CommandLine::new("sbxm prepare owner/repository").required_because("a single line")?;
    assert_eq!(command.as_str(), "sbxm prepare owner/repository");
    Ok(())
}

#[test]
fn an_empty_command_is_refused() {
    assert_eq!(CommandLine::new(""), Err(InvalidCommandLine::Empty));
    assert_eq!(CommandLine::new("   "), Err(InvalidCommandLine::Empty));
}

#[test]
fn a_line_feed_is_refused() {
    assert_eq!(
        CommandLine::new("sbxm prepare\nsbxm open"),
        Err(InvalidCommandLine::Multiline)
    );
    assert_eq!(
        CommandLine::new("sbxm prepare\n"),
        Err(InvalidCommandLine::Multiline)
    );
}

#[test]
fn a_carriage_return_is_refused() {
    assert_eq!(
        CommandLine::new("sbxm prepare\r\nsbxm open"),
        Err(InvalidCommandLine::Multiline)
    );
    assert_eq!(
        CommandLine::new("sbxm prepare\r"),
        Err(InvalidCommandLine::Multiline)
    );
}

#[test]
fn a_refused_command_becomes_a_block_that_is_left_out() {
    assert!(CommandLine::optional("").is_none());
    assert!(CommandLine::optional("sbxm ls").is_some());
}

#[test]
fn every_inline_fragment_exposes_the_plain_value_widths_are_measured_from() {
    assert_eq!(Inline::text("plain").as_str(), "plain");
    assert_eq!(Inline::important("owner/repo").as_str(), "owner/repo");
    assert_eq!(Inline::path("/tmp/example").as_str(), "/tmp/example");
    assert_eq!(
        Inline::state("running", VisualState::Positive).as_str(),
        "running"
    );
}

#[test]
fn rewriting_the_text_of_a_fragment_never_rewrites_what_the_value_is() {
    // 値を整えるのは組み立て側であり、整えたあとも「これは何か」は変わらない。
    assert_eq!(Inline::text("a\n").with_text("a"), Inline::text("a"));
    assert_eq!(
        Inline::important("owner/repo\n").with_text("owner/repo"),
        Inline::important("owner/repo")
    );
    assert_eq!(
        Inline::path("/tmp/x\n").with_text("/tmp/x"),
        Inline::path("/tmp/x")
    );
    assert_eq!(
        Inline::state("running\n", VisualState::Positive).with_text("running"),
        Inline::state("running", VisualState::Positive),
        "the semantic state is what decides the color, and it survives the rewrite"
    );
}

#[test]
fn the_same_value_can_carry_different_states_in_different_contexts() {
    // `stopped`は停止結果ならpositive、稼働要件ならattentionである。
    let finished = Inline::state("stopped", VisualState::Positive);
    let required = Inline::state("stopped", VisualState::Attention);
    assert_eq!(finished.as_str(), required.as_str());
    assert_ne!(finished, required);
}
