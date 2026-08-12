use crate::design::{Document, Field, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::support::disk::DiskObservation;

use crate::commands::present::Legend;
use crate::commands::status::project::ProjectStatus;

/// project scopeの`status`が並べるもの。
///
/// 指定案件だけを診断し、global環境の検査結果を混ぜない。
pub fn project_document(status: &ProjectStatus, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    let mut fields = vec![Field::new(
        msg!("status-item-project"),
        Inline::important(status.project.clone()),
    )];
    fields.extend(
        status
            .items
            .iter()
            .map(|item| Field::new(msg!(item.item), legend.project_status(item.value))),
    );

    let mut worktrees = Table::new(vec![
        msg!("column-path"),
        msg!("column-kind"),
        msg!("column-mode"),
        msg!("column-state"),
    ]);
    for worktree in &status.worktrees {
        worktrees.push(vec![
            Inline::path(worktree.path.clone()).into(),
            Inline::text(worktree.kind).into(),
            legend.project_status(worktree.mode).into(),
            legend.project_status(worktree.state).into(),
        ]);
    }

    let heading = msg!("status-worktrees-section");
    let document = Document::new().fields(Some(msg!("status-project-section")), fields);
    // 「worktreeが1本もない」という観測自体が診断結果であるため、空でもsectionを残す。
    let document = if worktrees.is_empty() {
        document.empty_section(Some(heading), msg!("status-no-worktrees"))
    } else {
        document.table(Some(heading), worktrees)
    };
    let document = disk_section(document, &status.disk);
    document.legend(Legend::heading(), legend.entries())
}

/// DISK sectionは、観測できた場合も理由だけの場合も常に表示する。
fn disk_section(document: Document, disk: &DiskObservation) -> Document {
    let heading = Some(msg!("status-disk-section"));
    match disk {
        DiskObservation::Observed(usage) => document.fields(
            heading,
            vec![
                Field::new(msg!("status-disk-mount"), Inline::path("/")),
                Field::new(
                    msg!("status-disk-free"),
                    Inline::text(format_kib(usage.free_kib)),
                ),
                Field::new(
                    msg!("status-disk-usable"),
                    Inline::text(format_kib(usage.usable_kib)),
                ),
                Field::new(
                    msg!("status-disk-capacity"),
                    Inline::text(format!("{}%", usage.capacity_percent)),
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

/// KiBをGiB単位の値へ、小数第1位まで四捨五入して表示する。
///
/// floatへ変換せず、u128の整数演算だけで丸める。
fn format_kib(kib: u64) -> String {
    const KIB_PER_GIB: u128 = 1024 * 1024;
    let tenths = (u128::from(kib) * 10 + KIB_PER_GIB / 2) / KIB_PER_GIB;
    format!("{}.{} GiB", tenths / 10, tenths % 10)
}
