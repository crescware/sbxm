//! testが前提とした値の取り出し。
//!
//! `unwrap`と`expect`はどちらもpanicで判定を終える。testの中では成立の確認そのものが
//! 目的であるため、成立しなかったことは失敗として返し、`?`で呼び出し元へ渡す。
//!
//! 失敗した場所は`#[track_caller]`が記録するため、行を探すのに追加の情報を要さない。

mod checked;
mod option;
mod refused;
mod required;
mod result;
mod unmet;

pub use checked::Checked;
pub use refused::Refused;
pub use required::Required;
pub use unmet::Unmet;
