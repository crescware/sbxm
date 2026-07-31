//! 表示幅と、列をそろえる整形。
//!
//! 幅はANSIを含まない元文字列から数える。装飾を付けたあとの文字列で数えると、色の
//! on/offで列の開始位置がずれる。整形を先に確定させ、装飾はそのあとで載せる。

mod display_width;
mod is_wide;
mod padding;
mod truncate;

pub(super) use display_width::display_width;
use is_wide::is_wide;
pub(super) use padding::padding;
pub(super) use truncate::truncate;

#[cfg(test)]
#[path = "width_test.rs"]
mod width_test;
