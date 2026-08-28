use super::PromptCapability;

#[test]
fn prompting_requires_both_input_and_output_to_be_terminals() {
    for ((stdin_is_tty, stderr_is_tty), expected) in [
        ((false, false), false),
        ((false, true), false),
        ((true, false), false),
        ((true, true), true),
    ] {
        assert_eq!(
            PromptCapability::from_streams(stdin_is_tty, stderr_is_tty).can_prompt(),
            expected,
            "stdin={stdin_is_tty}, stderr={stderr_is_tty}"
        );
    }
}
