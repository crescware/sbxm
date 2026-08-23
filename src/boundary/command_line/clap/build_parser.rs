use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::boundary::command_line::{
    ArgumentAction, ArgumentSyntax, Builder, CommandSyntax, PreparseOption,
};
use crate::design::ColorMode;
use crate::diagnostics::Result;
use crate::i18n::{Catalog, Locale};

pub(crate) fn build_parser(catalog: &Catalog, syntaxes: &[CommandSyntax]) -> Result<ClapCommand> {
    let builder = Builder::new(catalog);
    let templates = super::help_templates::HelpTemplates::new(&builder)?;
    let mut command = ClapCommand::new("sbxm")
        .about(builder.text("cli-about")?)
        .version(env!("CARGO_PKG_VERSION"))
        .help_template(templates.root())
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(false)
        .arg(lang_arg(&builder)?)
        .arg(color_arg(&builder)?)
        .arg(help_flag(&templates))
        .arg(
            Arg::new("version")
                .long("version")
                .short('V')
                .action(ArgAction::Version)
                .display_order(1001)
                .help(builder.text("cli-version-help")?),
        );

    for syntax in syntaxes {
        command = command.subcommand(command_syntax(&templates, syntax));
    }
    Ok(command)
}

fn command_syntax(
    templates: &super::help_templates::HelpTemplates,
    syntax: &CommandSyntax,
) -> ClapCommand {
    let mut command = ClapCommand::new(syntax.name)
        .about(syntax.about.clone())
        .help_template(templates.command(syntax))
        .disable_help_flag(true)
        .arg(help_flag(templates));

    for argument in &syntax.arguments {
        command = command.arg(argument_syntax(argument));
    }
    command
}

fn argument_syntax(syntax: &ArgumentSyntax) -> Arg {
    let mut argument = Arg::new(syntax.id).help(syntax.help.clone());
    if let Some(long) = syntax.long {
        argument = argument.long(long);
    }
    if let Some(short) = syntax.short {
        argument = argument.short(short);
    }
    if let Some(value_name) = syntax.value_name {
        argument = argument.value_name(value_name);
    }
    if syntax.required {
        argument = argument.required(true);
    }
    match syntax.action {
        ArgumentAction::Value => argument,
        ArgumentAction::Flag => argument.action(ArgAction::SetTrue),
        ArgumentAction::Append => argument.num_args(0..).action(ArgAction::Append),
    }
}

fn help_flag(templates: &super::help_templates::HelpTemplates) -> Arg {
    Arg::new("help")
        .long("help")
        .short('h')
        .action(ArgAction::Help)
        .display_order(1000)
        .help(templates.help_text().to_owned())
}

fn lang_arg(builder: &Builder) -> Result<Arg> {
    let option_name = PreparseOption::Lang.option_name();
    Ok(Arg::new(option_name)
        .long(option_name)
        .global(true)
        .value_name(Locale::value_name())
        .value_parser(PossibleValuesParser::new(Locale::accepted_values()))
        .hide_possible_values(true)
        .display_order(900)
        .help(builder.message(&crate::msg!(
            "cli-lang-help",
            supported = Locale::value_list()
        ))?))
}

fn color_arg(builder: &Builder) -> Result<Arg> {
    let option_name = PreparseOption::Color.option_name();
    Ok(Arg::new(option_name)
        .long(option_name)
        .global(true)
        .value_name(ColorMode::value_name())
        .value_parser(PossibleValuesParser::new(ColorMode::accepted_values()))
        .hide_possible_values(true)
        .display_order(901)
        .help(builder.message(&crate::msg!(
            "cli-color-help",
            supported = ColorMode::value_list()
        ))?))
}
