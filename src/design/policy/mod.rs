//! 色と文字集合を出すかどうかの判定。
//!
//! 判定は純粋関数へ閉じ、`std::env`と`IsTerminal`を読むのは本moduleのadapterだけとする。
//! 判定そのものを環境から切り離さないと、testが全分岐を確かめるためにprocess全体の
//! 環境変数を書き換えることになり、並行実行で結果が混ざる。
//!
//! streamごとに独立して判定する。stdoutをpipeしてstderrを端末に残した場合、正常結果は
//! plain text、進捗と診断は色付きになる。

mod character_set;
mod color_mode;
mod color_setting;
mod environment;
mod rendering_policy;
mod stream_policy;
mod terminals;

pub use character_set::CharacterSet;
pub use color_mode::ColorMode;
pub use color_setting::ColorSetting;
pub use environment::Environment;
pub use rendering_policy::RenderingPolicy;
pub use stream_policy::StreamPolicy;
pub use terminals::Terminals;

#[cfg(test)]
#[path = "policy_test.rs"]
mod policy_test;
