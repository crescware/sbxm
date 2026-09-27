use super::*;

impl PtyConfirmedCommand {
    /// 答えるべき確認prompt。testのhostが答えを再生するために使う。
    pub(crate) fn expected_prompt(&self) -> &str {
        &self.expected_prompt
    }
}

#[test]
fn the_expected_prompt_is_what_new_was_given() {
    let command = PtyConfirmedCommand::new("sbx", &[], "the sandbox", "confirmation");
    assert_eq!(command.expected_prompt(), "confirmation");
}
