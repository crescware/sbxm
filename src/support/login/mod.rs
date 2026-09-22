//! 案件を選ぶ前にDocker Sandboxesへの認証を確認する。

mod require_signed_in;

pub use require_signed_in::require_signed_in;

#[cfg(test)]
#[path = "login_test.rs"]
mod login_test;
