//! アプリケーションの起動処理。
//!
//! process境界から受け取った引数をもとに、設定、表示、CLI、commandを組み立てる。
//! 個別commandの責務は`crate::commands`に置き、ここではapplication全体の組み合わせだけを行う。

mod execute;
mod invocation;
mod report_startup_error;
mod run;

pub(crate) use run::run;

#[cfg(test)]
pub(crate) use invocation::{
    TestInteractivity as Interactivity, build_parser_for_test, parse_for_test,
};
