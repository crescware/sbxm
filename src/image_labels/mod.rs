//! image configが宣言するlabel。
//!
//! configのどのkeyがlabelを持つかは呼び出し側が決める。ここは取り出した値の
//! 形だけを見る。

mod label_defect;
mod labels_from_declared;

pub use label_defect::LabelDefect;
pub use labels_from_declared::labels_from_declared;

#[cfg(test)]
#[path = "image_labels_test.rs"]
mod image_labels_test;
