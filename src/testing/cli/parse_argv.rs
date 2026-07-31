use crate::cli::{Interactivity, Outcome, parse};
use crate::diagnostics::Result;
use crate::i18n::{Catalog, Locale};

use super::argv;

/// 正本localeでargvをparseする。
pub fn parse_argv(arguments: &[&str], interactivity: Interactivity) -> Result<Outcome> {
    let catalog = Catalog::new(Locale::En);
    parse(&argv(arguments), &catalog, interactivity)
}
