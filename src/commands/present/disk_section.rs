use crate::design::Document;
use crate::msg;

use crate::support::disk::{DiskObservation, disk_meter};

/// DISK sectionを足す。観測できた場合も理由だけの場合も、常に何かを表示する。
///
/// `status`と`open`のどちらも、同じ観測結果を同じ語彙で示す。
pub fn disk_section(document: Document, disk: &DiskObservation) -> Document {
    let heading = Some(msg!("status-disk-section"));
    match disk {
        DiskObservation::Observed(usage) => match disk_meter(usage) {
            Some(meter) => document.capacity(msg!("status-disk-section"), meter),
            None => document.empty_section(heading, msg!("status-disk-parse-failed")),
        },
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

#[cfg(test)]
#[path = "disk_section_test.rs"]
mod disk_section_test;
