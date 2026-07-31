#![doc = include_str!("README.md")]

mod blank;
mod block;
mod cell;
pub mod diagnostic;
pub mod document;
mod field;
mod guidance;
mod guidance_item;
mod legend_entry;
mod line;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod paint;
mod painter;
pub mod policy;
mod progress_sink;
pub mod prompt;
pub mod renderer;
mod row;
mod section;
mod section_body;
#[cfg(test)]
mod silent_progress;
pub mod style;
pub mod table;
pub mod text;
mod trailing;
mod ui;
mod width;

use blank::blank;
pub use block::Block;
pub use cell::Cell;
pub use diagnostic::{Remediation, Warning};
pub use document::Document;
pub use field::Field;
pub use guidance::Guidance;
pub use guidance_item::GuidanceItem;
pub use legend_entry::LegendEntry;
use line::line;
use paint::paint;
use painter::Painter;
pub use policy::{ColorMode, Environment, OutputPolicy, Terminals};
pub use progress_sink::ProgressSink;
pub use prompt::PromptUi;
use row::row;
pub use section::Section;
pub use section_body::SectionBody;
#[cfg(test)]
pub use silent_progress::SilentProgress;
pub use style::VisualState;
pub use table::Table;
pub use text::{CommandLine, Inline};
use trailing::Trailing;
pub use ui::Ui;
