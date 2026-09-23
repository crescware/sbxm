use crate::design::{Document, Field, GuidanceItem, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::present::{self, Legend};
use crate::commands::status::project::{FileState, ProjectStatus};

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
        msg!("column-remote"),
    ]);
    for worktree in &status.worktrees {
        let remote = worktree.remote.display();
        legend.add(&remote, worktree.remote.legend_id());
        worktrees.push(vec![
            Inline::path(worktree.path.clone()).into(),
            Inline::text(worktree.kind).into(),
            legend.project_status(worktree.mode).into(),
            legend.project_status(worktree.state).into(),
            Inline::text(remote).into(),
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
    let document = declared_files(document, status, &mut legend);
    let document = present::disk_section(document, &status.disk);
    let document = document.legend(Legend::heading(), legend.entries());
    next_step(document, status)
}

/// 宣言fileごとの、hostとSandboxの状態。宣言が無ければsectionごと省く。
///
/// hostで変わったfileがあれば、それを置く手順を示す。Sandboxで書き換えられたfileは
/// `apply --files`が置き換えないことも述べる。
fn declared_files(document: Document, status: &ProjectStatus, legend: &mut Legend) -> Document {
    if status.files.is_empty() {
        return document;
    }
    let mut table = Table::new(vec![
        msg!("column-destination"),
        msg!("column-host"),
        msg!("column-sandbox"),
    ]);
    for file in &status.files {
        table.push(vec![
            Inline::path(file.destination.clone()).into(),
            legend.file_state(file.host).into(),
            legend.file_state(file.sandbox).into(),
        ]);
    }
    let mut document = document.table(Some(msg!("status-files-section")), table);
    if status
        .files
        .iter()
        .any(|file| matches!(file.sandbox, FileState::Modified | FileState::Unrecorded))
    {
        document = document.note(msg!("status-files-modified-note"));
    }
    if status.files.iter().any(|file| {
        matches!(file.host, FileState::Updated | FileState::Unplaced)
            && file.sandbox != FileState::NotApplicable
    }) {
        document = document
            .note(msg!("status-files-apply-note"))
            .try_command(format!("sbxm apply {} --files", status.project));
    }
    document
}

/// 今すぐ実行できる1手を、末尾に1回だけ示す。
///
/// 相反するcommandを並べない。復旧が要る案件では、Dockerfileが変わっていても
/// `repair`だけを出し、そのあとに世代交代が続き得ることは説明として添える。
fn next_step(document: Document, status: &ProjectStatus) -> Document {
    let Some(next) = status.next else {
        return document;
    };
    let mut items = vec![GuidanceItem::Plain(msg!(next.reason_id()))];
    if next.leaves_generation_behind() {
        items.push(GuidanceItem::Plain(msg!("guidance-next-then-status")));
    }
    // 案件IDを打ち直させない。次のcommandはそのままcopyできる形で出す。
    document
        .guidance(Some(msg!("status-next-heading")), items)
        .try_command(next.command(&status.project))
}
