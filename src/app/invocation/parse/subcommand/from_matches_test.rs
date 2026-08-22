//! 解析済みsubcommandの組み立て。

use crate::diagnostics::ErrorId;
use crate::testing::cli::tty;
use crate::testing::outcome::{Checked, Refused, Required};

use super::from_matches;

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
