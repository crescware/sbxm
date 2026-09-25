//! Sandboxからhostへのbyte列の受け取り。
//!
//! 受け取った内容は信用しない。まずproject配下の隔離領域へだけ書き、既存のfileを直接
//! 上書きしない。受け取った内容をどこへ採用するかは、それを使う側が確かめてから決める。

mod receive;

pub use receive::receive;

#[cfg(test)]
#[path = "retrieve_test.rs"]
mod retrieve_test;
