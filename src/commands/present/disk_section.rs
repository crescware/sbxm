use crate::design::{Document, Field, Inline};
use crate::msg;

use crate::support::disk::{DiskObservation, format_gib, format_percent};

/// DISK sectionを足す。観測できた場合も理由だけの場合も、常に何かを表示する。
///
/// `status`と`open`のどちらも、同じ観測結果を同じ語彙で示す。
pub fn disk_section(document: Document, disk: &DiskObservation) -> Document {
    let heading = Some(msg!("status-disk-section"));
    match disk {
        DiskObservation::Observed(usage) => document.fields(
            heading,
            vec![
                Field::new(msg!("status-disk-mount"), Inline::path("/")),
                Field::new(
                    msg!("status-disk-free"),
                    Inline::text(format_gib(usage.free_kib)),
                ),
                Field::new(
                    msg!("status-disk-usable"),
                    Inline::text(format_gib(usage.usable_kib)),
                ),
                Field::new(
                    msg!("status-disk-capacity"),
                    Inline::text(format_percent(usage.capacity_percent)),
                ),
            ],
        ),
        DiskObservation::NotObservedStopped => {
            document.empty_section(heading, msg!("status-disk-not-observed-stopped"))
        }
        DiskObservation::NotObservedNotCreated => {
            document.empty_section(heading, msg!("status-disk-not-observed-not-created"))
        }
        DiskObservation::NotObservedMismatch => {
            document.empty_section(heading, msg!("status-disk-not-observed-mismatch"))
        }
        DiskObservation::CommandMissing => {
            document.empty_section(heading, msg!("status-disk-command-missing"))
        }
        DiskObservation::ParseFailed => {
            document.empty_section(heading, msg!("status-disk-parse-failed"))
        }
    }
}
