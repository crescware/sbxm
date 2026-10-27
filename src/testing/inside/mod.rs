//! まとめた確認（`INSIDE_CHECK`）への、fakeのhostの答え方。
//!
//! 実物ではSandboxのshellがcommandごとの結果を並べる。fakeは、1件ずつ起動されたときと
//! 同じ答えをcommandごとに作り、同じ形に並べて返す。引数の読み方は本番と同じ
//! `split_inside_check`を使う。

mod frame_inside_check;

pub use crate::boundary::host::protocol::split_inside_check;
pub use frame_inside_check::frame_inside_check;
