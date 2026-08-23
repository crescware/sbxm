use super::ParsedCommand;

/// help/versionを含む、parser結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommandLine {
    Help(String),
    Version(String),
    Command(ParsedCommand),
}
