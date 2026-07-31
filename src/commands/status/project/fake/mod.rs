//! `status <project>`のtestが使う応答と、結果の読み取り。

mod looking_inside;
mod three_entries;
mod value_of;
mod without_image;

pub use looking_inside::looking_inside;
pub use three_entries::three_entries;
pub use value_of::value_of;
pub use without_image::without_image;
