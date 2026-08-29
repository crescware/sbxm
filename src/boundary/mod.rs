//! application と外界の境界。
//!
//! host process の実行と、その process が返す外部 protocol の解釈をここへ集める。
//! application と support には、外界固有の transport や出力形式ではなく、境界で
//! 変換された値だけを渡す。

pub mod command_line;
pub mod host;
pub(crate) mod terminal;
