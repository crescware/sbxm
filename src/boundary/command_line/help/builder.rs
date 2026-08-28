use crate::boundary::command_line::CommandSyntax;
use crate::diagnostics::{Msg, Result};
use crate::i18n::Catalog;

use super::{format, text};

/// 選択したlocaleでparser非依存のcommand syntaxを組み立てる。
pub struct Builder<'a> {
    catalog: &'a Catalog,
}

impl<'a> Builder<'a> {
    pub fn new(catalog: &'a Catalog) -> Builder<'a> {
        Builder { catalog }
    }

    pub fn text(&self, id: &'static str) -> Result<String> {
        text(self.catalog, id)
    }

    pub fn message(&self, message: &Msg) -> Result<String> {
        format(self.catalog, message)
    }

    pub fn command(&self, name: &'static str, about_id: &'static str) -> Result<CommandSyntax> {
        Ok(CommandSyntax::new(name, self.text(about_id)?))
    }
}
