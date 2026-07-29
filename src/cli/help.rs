//! helpの組み立て。
//!
//! CLI parser libraryの英語固定表記へ委ねず、選択したlocaleでheadingとhelp textを
//! 組む。templateはcommandが持つ引数の種類ごとに決まる。

use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::error::{Error, ErrorId, Msg, Result};
use crate::i18n::Catalog;
use crate::msg;

/// 選択したlocaleでcommandを組み立てる。
///
/// 各commandはここからhelpの器を受け取り、自分の引数だけを足す。headingとhelpの
/// 見せ方を1箇所へ集め、commandの数だけ同じ組み立てを書かない。
pub struct Builder<'a> {
    catalog: &'a Catalog,
    /// subcommandを並べるroot command用。
    root: String,
    /// optionだけを持つcommand用。
    leaf: String,
    /// 位置引数を持つcommand用。
    positional: String,
    /// `--help`自身のhelp text。
    help: String,
}

impl<'a> Builder<'a> {
    pub fn new(catalog: &'a Catalog) -> Result<Builder<'a>> {
        let usage = text(catalog, "cli-heading-usage")?;
        let commands = text(catalog, "cli-heading-commands")?;
        let options = text(catalog, "cli-heading-options")?;
        let arguments = text(catalog, "cli-heading-arguments")?;

        Ok(Builder {
            catalog,
            root: format!(
                "{{about}}\n\n{usage} {{usage}}\n\n{commands}\n{{subcommands}}\n\n{options}\n{{options}}"
            ),
            leaf: format!("{{about}}\n\n{usage} {{usage}}\n\n{options}\n{{options}}"),
            positional: format!(
                "{{about}}\n\n{usage} {{usage}}\n\n{arguments}\n{{positionals}}\n\n{options}\n{{options}}"
            ),
            help: text(catalog, "cli-help-help")?,
        })
    }

    pub fn text(&self, id: &'static str) -> Result<String> {
        text(self.catalog, id)
    }

    pub fn message(&self, message: &Msg) -> Result<String> {
        format(self.catalog, message)
    }

    pub fn root_template(&self) -> String {
        self.root.clone()
    }

    /// helpとversionは、commandごとのoptionより後に並べる。
    pub fn help_flag(&self) -> Arg {
        Arg::new("help")
            .long("help")
            .short('h')
            .action(ArgAction::Help)
            .display_order(1000)
            .help(self.help.clone())
    }

    /// optionだけを持つcommandの器。
    pub fn leaf(&self, name: &'static str, about_id: &'static str) -> Result<ClapCommand> {
        self.shell(name, about_id, self.leaf.clone())
    }

    /// 位置引数を持つcommandの器。
    pub fn positional(&self, name: &'static str, about_id: &'static str) -> Result<ClapCommand> {
        self.shell(name, about_id, self.positional.clone())
    }

    fn shell(
        &self,
        name: &'static str,
        about_id: &'static str,
        template: String,
    ) -> Result<ClapCommand> {
        Ok(ClapCommand::new(name)
            .about(self.text(about_id)?)
            .help_template(template)
            .disable_help_flag(true)
            .arg(self.help_flag()))
    }
}

fn text(catalog: &Catalog, id: &'static str) -> Result<String> {
    format(catalog, &msg!(id))
}

fn format(catalog: &Catalog, message: &Msg) -> Result<String> {
    catalog.format(message).map_err(|failure| {
        Error::new(
            ErrorId::MessageFormatFailed,
            msg!("error-invalid-arguments").with("detail", failure),
        )
    })
}
