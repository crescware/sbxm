//! `rebuild`の出力。
//!
//! 適用した世代を1行で述べ、warningは同じblockへ混ぜず別に残す。どちらをどのstreamへ
//! 出すかと、そのあとのexit codeまでをここが決める。

mod document;
mod report;

pub use document::document;
pub use report::report;
