//! `prepare`のtestが動かすSandbox世界のfake。
//!
//! 工程は外部commandの応答だけで決まるため、応答と観測できる状態をここが持つ。

mod answers;
mod bench;
mod image_id;
mod inside;
mod sandbox_row;
mod world;

pub use bench::Bench;
pub use image_id::IMAGE_ID;
pub use sandbox_row::SandboxRow;
pub use world::World;
