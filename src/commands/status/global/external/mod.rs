//! 外部commandの実行結果の読み取り。

mod describe;
mod external_of;
mod read_stdout;

pub(super) use describe::describe;
pub(super) use external_of::external_of;
pub(super) use read_stdout::read_stdout;
