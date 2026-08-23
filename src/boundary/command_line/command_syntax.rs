use super::{ArgumentSyntax, CommandLayout};

/// application commandが公開するparser非依存のsyntax。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSyntax {
    pub(crate) name: &'static str,
    pub(crate) about: String,
    pub(crate) layout: CommandLayout,
    pub(crate) arguments: Vec<ArgumentSyntax>,
}

impl CommandSyntax {
    pub(crate) fn new(name: &'static str, about: String, layout: CommandLayout) -> CommandSyntax {
        CommandSyntax {
            name,
            about,
            layout,
            arguments: Vec::new(),
        }
    }

    pub fn arg(mut self, argument: ArgumentSyntax) -> Self {
        self.arguments.push(argument);
        self
    }
}
