//! sbxm。案件ごとのDocker Sandboxを構築、接続、診断、破棄するCLI。
//!
//! 実行順は、引数validation、config load、project解決、外部command、mutationとする。
//! commandの実装は`commands`が1 command 1 directoryで持ち、本fileはprocess境界から
//! applicationへ引き渡すだけを行う。
//!
//! 利用者向けの描画はすべて`design`が行う。本fileはstreamへ直接書かない。

mod app;
mod archive;
mod cli;
mod command;
mod commands;
mod compatibility;
mod config;
mod design;
mod diagnostics;
mod git;
mod hash;
mod i18n;
mod image_labels;
mod metadata;
mod paths;
mod project;
mod registry;
mod repository;
mod support;
#[cfg(test)]
mod testing;

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let code = app::run(argv);
    std::process::ExitCode::from(code.as_u8())
}
