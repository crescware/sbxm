//! `add`の出力。
//!
//! 登録とhost cloneまでを示し、次にやることを続けて出す。GitHub tokenの登録先は
//! Sandbox名であり、その名前はここで確定する。
//!
//! 実行を求めるcommandは説明文へ混ぜず、独立blockとして渡す。番号は説明行に付き、
//! command行には付かない。

mod document;

pub use document::document;
