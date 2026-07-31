//! `ls`の出力。
//!
//! 一覧そのものが結論であるためsummaryを足さない。管理案件と管理外Sandboxは別のsection
//! とし、管理外が1件もなければそのsectionごと省く。
//!
//! registryが指すpathが消えていても行を落とさない。観測した状態をそのまま示し、
//! 復旧に必要なentryを一覧から失わせない。

mod document;

pub use document::document;
