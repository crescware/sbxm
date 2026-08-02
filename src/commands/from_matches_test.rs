//! 解析済みsubcommandの組み立て。
//!
//! parserが受け入れた名前と、dispatcherが知っている名前は別に育つ。片方だけ増えた場合に
//! 静かに何も起きないことがないよう、知らない名前は拒否として現れる。

use crate::diagnostics::ErrorId;

use crate::testing::cli::tty;
use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

#[test]
fn a_subcommand_the_dispatcher_does_not_know_is_refused_and_named() -> Checked {
    let matches = clap::Command::new("sbxm")
        .try_get_matches_from(["sbxm"])
        .required_because("an empty set of arguments parses")?;

    let error = from_matches("teleport", &matches, tty())
        .refused_because("a subcommand with no arm cannot be run")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal is diagnosed")?;
    assert_eq!(diagnostic.id, ErrorId::UnknownSubcommand);
    // どの名前を組み立てられなかったかを、利用者の書いた綴りのまま示す。
    assert!(
        diagnostic
            .description
            .args
            .contains(&("subcommand", "teleport".to_string())),
        "{:?}",
        diagnostic.description.args
    );
    Ok(())
}
