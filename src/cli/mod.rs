//! CLI parse。
//!
//! CLI parser libraryの自動help・自動終了へlocale決定を委ねず、選択したlocaleで
//! help、usage、parse errorを生成する。validationは次の順で行う。
//!
//! 1. syntaxとoption関係
//! 2. `--lang`
//! 3. command固有の引数
//! 4. config load
//! 5. project解決
//! 6. 外部command
//! 7. mutation
//!
//! 本moduleは1から3までを担当し、config、filesystem、外部状態には触れない。
//! 3のcommand固有部分は`crate::commands`の各commandが持つ。

mod build_command;
mod color;
mod diagnostics;
pub mod help;
mod interactivity;
mod lang;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod outcome;
mod parse;
pub mod project_arg;
mod version_line;

use build_command::build_command;
pub use color::peek_color;
pub use help::Builder;
pub use interactivity::Interactivity;
pub use lang::{PeekedLang, invalid_lang_error, peek_lang};
pub use outcome::Outcome;
pub use parse::parse;
pub use version_line::version_line;
