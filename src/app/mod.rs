//! アプリケーションの起動処理。
//!
//! process境界から受け取った引数をもとに、設定、表示、CLI、commandを組み立てる。
//! 個別commandの責務は`crate::commands`に置き、ここではapplication全体の組み合わせだけを行う。

mod resolve_display_locale;
mod run;

pub(crate) use run::run;
