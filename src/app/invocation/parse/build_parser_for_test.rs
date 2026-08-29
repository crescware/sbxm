use crate::boundary::command_line::Builder;
use crate::commands::Command;
use crate::diagnostics::Result;
use crate::i18n::Catalog;

pub(crate) fn build_parser_for_test(catalog: &Catalog) -> Result<clap::Command> {
    let builder = Builder::new(catalog);
    let syntax = Command::syntax(&builder)?;
    crate::boundary::command_line::clap::build_parser_for_test(catalog, &syntax)
}
