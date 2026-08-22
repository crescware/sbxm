use crate::app::{Interactivity, parse_for_test};
use crate::commands::Command;
use crate::diagnostics::Result;
use crate::i18n::{Catalog, Locale};

use super::argv;

/// 正本localeでargvをparseする。
pub fn parse_argv(arguments: &[&str], interactivity: Interactivity) -> Result<Command> {
    let catalog = Catalog::new(Locale::En);
    parse_for_test(&argv(arguments), &catalog, interactivity)
}
