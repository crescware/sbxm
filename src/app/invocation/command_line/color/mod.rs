//! `--color`。
//!
//! 描画条件はhelpを組み立てるより前に決まっている必要があるため、`--lang`と同じく
//! argvから副作用なく先読みする。受け付ける値と表記は[`ColorMode`]の宣言から導出し、
//! 本moduleは判定を持たない。

mod arg;
mod peek_color;

pub use arg::arg;
pub use peek_color::peek_color;

#[cfg(test)]
#[path = "color_test.rs"]
mod color_test;
