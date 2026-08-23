use crate::boundary::command_line::{CommandLayout, CommandSyntax};
use crate::diagnostics::{Msg, Result};
use crate::i18n::Catalog;

use super::{format, text};

/// 選択したlocaleでparser非依存のcommand syntaxを組み立てる。
pub struct Builder<'a> {
    catalog: &'a Catalog,
    root: String,
    leaf: String,
    positional: String,
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

    pub(crate) fn root_template(&self) -> String {
        self.root.clone()
    }

    pub(crate) fn template(&self, layout: CommandLayout) -> String {
        match layout {
            CommandLayout::Leaf => self.leaf.clone(),
            CommandLayout::Positional => self.positional.clone(),
        }
    }

    pub(crate) fn help_text(&self) -> &str {
        &self.help
    }

    pub fn command(
        &self,
        name: &'static str,
        about_id: &'static str,
        layout: CommandLayout,
    ) -> Result<CommandSyntax> {
        Ok(CommandSyntax::new(name, self.text(about_id)?, layout))
    }
}
