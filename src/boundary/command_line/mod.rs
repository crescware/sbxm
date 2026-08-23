//! parser非依存のcommand-line境界。
//!
//! `CommandSyntax`と`Arguments`だけがapplication commandへ渡る。clapの型、error、help
//! renderingは[`clap`]へ閉じ込める。

mod argument_action;
mod argument_syntax;
mod arguments;
pub(crate) mod clap;
mod command_layout;
// `CommandLine`の実装はfile名とsubjectを一致させるmodule境界規約に従う。
#[allow(clippy::module_inception)]
mod command_line;
mod command_syntax;
pub mod help;
mod invalid_lang_error;
mod locale_override;
mod parsed_command;
mod parsed_command_line;
mod peek;
mod preparse_option;

pub use argument_action::ArgumentAction;
pub use argument_syntax::ArgumentSyntax;
pub use arguments::Arguments;
pub(crate) use clap::parse;
pub use command_layout::CommandLayout;
pub(crate) use command_line::CommandLine;
pub use command_syntax::CommandSyntax;
pub use help::Builder;
pub use parsed_command::ParsedCommand;
pub use parsed_command_line::ParsedCommandLine;
pub(crate) use preparse_option::PreparseOption;
