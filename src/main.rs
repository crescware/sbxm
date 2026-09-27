//! sbxm。案件ごとのDocker Sandboxを構築、接続、診断、破棄するCLI。
//!
//! 実行順は、引数validation、config load、project解決、外部command、mutationとする。
//! commandの実装は`commands`が1 command 1 directoryで持ち、本fileはprocess境界から
//! applicationへ引き渡すだけを行う。
//!
//! 利用者向けの描画はすべて`design`が行う。本fileはstreamへ直接書かない。

// test harnessは、test 1件につき1つのpointerを並べた配列を生成する。unit testが2048件を
// 超えるとその配列が16KiBを超え、`large_stack_arrays`が場所を示さずに警告する。本番の
// codeには関わらないため、test buildでだけ外す。
#![cfg_attr(test, allow(clippy::large_stack_arrays))]

mod app;
mod archive;
mod boundary;
mod commands;
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
mod time;

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let code = app::run(argv);
    std::process::ExitCode::from(code.as_u8())
}
