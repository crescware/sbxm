use crate::boundary::command_line::{ArgumentSyntax, Builder, CommandSyntax};
use crate::diagnostics::Result;

/// clapのhelp templateと、clapが使うplaceholderをここへ閉じ込める。
pub(super) struct HelpTemplates {
    root: String,
    leaf: String,
    positional: String,
    help: String,
}

impl HelpTemplates {
    pub(super) fn new(builder: &Builder) -> Result<HelpTemplates> {
        let usage = builder.text("cli-heading-usage")?;
        let commands = builder.text("cli-heading-commands")?;
        let options = builder.text("cli-heading-options")?;
        let arguments = builder.text("cli-heading-arguments")?;

        Ok(HelpTemplates {
            root: format!(
                "{{about}}\n\n{usage} {{usage}}\n\n{commands}\n{{subcommands}}\n\n{options}\n{{options}}"
            ),
            leaf: format!("{{about}}\n\n{usage} {{usage}}\n\n{options}\n{{options}}"),
            positional: format!(
                "{{about}}\n\n{usage} {{usage}}\n\n{arguments}\n{{positionals}}\n\n{options}\n{{options}}"
            ),
            help: builder.text("cli-help-help")?,
        })
    }

    pub(super) fn root(&self) -> String {
        self.root.clone()
    }

    pub(super) fn command(&self, syntax: &CommandSyntax) -> String {
        match CommandLayout::from(syntax) {
            CommandLayout::Leaf => self.leaf.clone(),
            CommandLayout::Positional => self.positional.clone(),
        }
    }

    pub(super) fn help_text(&self) -> &str {
        &self.help
    }
}

/// command syntaxから導出するclap固有のhelp layout。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandLayout {
    Leaf,
    Positional,
}

impl CommandLayout {
    fn from(syntax: &CommandSyntax) -> CommandLayout {
        if syntax.arguments.iter().any(ArgumentSyntax::is_positional) {
            CommandLayout::Positional
        } else {
            CommandLayout::Leaf
        }
    }
}
