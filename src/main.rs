//! sbxm。案件ごとのDocker Sandboxを構築、接続、診断、破棄するCLI。
//!
//! Phase 1では共通基盤、`init`、`status --global`を実装する。

// Phase 1の成果物には、後続Phaseのworkflowが判断を追加せずに利用する共通型、path導出、
// 永続化、外部command実行、互換性probeが含まれる。最初の呼び出し側がPhase 2以降で入る
// まで、それらはbinary本体からは参照されない。
#![allow(dead_code)]

mod cli;
mod command;
mod compatibility;
mod config;
mod error;
mod i18n;
mod paths;
mod project;
mod workflow;

fn main() {}
