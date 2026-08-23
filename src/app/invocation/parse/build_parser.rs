use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::diagnostics::Result;
use crate::i18n::Catalog;

use super::help::Builder;

/// `FTLからhelp` textを組み立てたparserを作る。
pub(crate) fn build_parser(catalog: &Catalog) -> Result<ClapCommand> {
    let builder = Builder::new(catalog)?;

    let mut command = ClapCommand::new("sbxm")
        .about(builder.text("cli-about")?)
        .version(env!("CARGO_PKG_VERSION"))
        .help_template(builder.root_template())
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(false)
        .arg(super::super::CommandLine::lang_arg(&builder)?)
        .arg(super::super::CommandLine::color_arg(&builder)?)
        .arg(builder.help_flag())
        .arg(
            Arg::new("version")
                .long("version")
                .short('V')
                .action(ArgAction::Version)
                .display_order(1001)
                .help(builder.text("cli-version-help")?),
        );

    // 並び順がhelpの並び順になる。
    for subcommand in super::subcommand::Subcommand::specs(&builder)? {
        command = command.subcommand(subcommand);
    }
    Ok(command)
}
