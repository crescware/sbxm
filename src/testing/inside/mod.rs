//! まとめた確認（`INSIDE_CHECK`）への、fakeのhostの答え方。
//!
//! 実物ではSandboxのshellがcommandごとの結果を並べる。fakeは、1件ずつ起動されたときと
//! 同じ答えをcommandごとに作り、同じ形に並べて返す。

mod frame_inside_check;
mod split_inside_check;

pub use frame_inside_check::frame_inside_check;
pub use split_inside_check::split_inside_check;
