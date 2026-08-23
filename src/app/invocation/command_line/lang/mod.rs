//! `--lang`。
//!
//! 受け付ける値も表示も組み込みlocaleの定義から導出する。言語を増やしてもこのmoduleを
//! 触らない。

mod arg;
mod invalid_lang_error;
mod option_name;
mod peeked_lang;
mod tag_list;
mod tags;
mod value_name;

pub use arg::arg;
pub use invalid_lang_error::invalid_lang_error;
pub(super) use option_name::OPTION_NAME;
pub use peeked_lang::PeekedLang;
use tag_list::tag_list;
use tags::tags;
use value_name::value_name;

#[cfg(test)]
#[path = "lang_test.rs"]
mod lang_test;
