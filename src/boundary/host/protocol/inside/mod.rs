//! Sandboxの中の確認を1回の`sbx exec`にまとめる手順と、その出力の解釈。
//!
//! 手順はhostが渡し、出力はSandboxのshellが並べる。並べ方はhostとSandboxのあいだの
//! 契約であり、ここだけが両側の形を決める。

mod inside_check;
mod inside_check_arguments;
mod inside_record;
mod parse_inside_check;

pub use inside_check::INSIDE_CHECK;
pub use inside_check_arguments::inside_check_arguments;
pub use inside_record::InsideRecord;
pub use parse_inside_check::parse_inside_check;

#[cfg(test)]
#[path = "inside_test.rs"]
mod inside_test;
