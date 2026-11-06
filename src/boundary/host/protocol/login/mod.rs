//! Docker Sandboxes自身が返した認証エラーの解釈。

mod is_login_missing;
mod is_login_unavailable;

pub use is_login_missing::is_login_missing;
pub use is_login_unavailable::is_login_unavailable;

#[cfg(test)]
#[path = "login_test.rs"]
mod login_test;
