use super::Arguments;

/// parserが選んだcommandと、そのcommandへ渡す中立なvalues。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub name: String,
    pub arguments: Arguments,
}
