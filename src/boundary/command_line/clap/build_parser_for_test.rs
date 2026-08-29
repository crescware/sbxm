use crate::boundary::command_line::CommandSyntax;
use crate::diagnostics::Result;
use crate::i18n::Catalog;

pub(crate) fn build_parser_for_test(
    catalog: &Catalog,
    syntaxes: &[CommandSyntax],
) -> Result<clap::Command> {
    super::build_parser::build_parser(catalog, syntaxes)
}
