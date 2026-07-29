//! Commandの出力が共有する、翻訳と凡例の小さな助け。
//!
//! FTLのformatに失敗しても出力自体は止めず、失敗したmessage IDとlocaleが分かる文字列を
//! 代わりに返す。

use crate::compatibility::SandboxState;
use crate::error::{Error, Msg};
use crate::i18n::Catalog;
use crate::metadata::CreationMode;

use super::Reporter;
use super::files::Placement;
use super::tools::Note;

pub fn report(catalog: &Catalog, error: &Error) {
    let reporter = Reporter::new(catalog);
    reporter.print_error(error, &mut std::io::stderr());
}

pub fn format_or_report(catalog: &Catalog, message: &Msg) -> String {
    catalog
        .format(message)
        .unwrap_or_else(|failure| failure.to_string())
}

pub fn text_or_report(catalog: &Catalog, id: &str) -> String {
    catalog
        .text(id)
        .unwrap_or_else(|failure| failure.to_string())
}

/// Sandboxに入っているtoolが返した案内。
///
/// どのtoolが返したかは印字しない。sbxmが代わりに実行しないことを示す文面そのものが
/// toolを名乗る。
pub fn print_notes(catalog: &Catalog, notes: &[Note]) {
    for note in notes {
        println!("\n{}", format_or_report(catalog, &note.heading));
        for item in &note.items {
            println!("  {item}");
        }
        println!("{}", format_or_report(catalog, &note.hint));
    }
}

/// 状態値の凡例。
pub fn sandbox_state_legend(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "legend-sandbox-running",
        SandboxState::Stopped => "legend-sandbox-stopped",
    }
}

pub fn mode_legend(mode: CreationMode) -> (&'static str, &'static str) {
    match mode {
        CreationMode::Attached => ("attached", "legend-attached"),
        CreationMode::Detached => ("detached", "legend-detached"),
    }
}

pub fn placement_legend(placement: Placement) -> (&'static str, &'static str) {
    match placement {
        Placement::Placed => ("placed", "legend-placed"),
        Placement::Unchanged => ("unchanged", "legend-unchanged"),
    }
}
