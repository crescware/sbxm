//! applicationと外界の境界。
//!
//! processの外側に接続する具体adapterを集める。
//!
//! clap、実terminal、host processの具体的な型はこのmoduleの内側へ閉じ込め、application
//! commandへは境界の値だけを渡す。外部toolのprotocolを解釈する型は、複数のworkflowが
//! 共有するため`compatibility`に別の分類軸として置く。

pub mod command_line;
pub mod host;
pub(crate) mod terminal;
