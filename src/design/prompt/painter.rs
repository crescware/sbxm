use std::ops::Range;

use crate::diagnostics::Msg;
use crate::i18n::Catalog;
use crate::msg;

use crate::design::policy::StreamPolicy;
use crate::design::style::{self, GlyphSet, Role};
use crate::design::width::{display_width, shorten, truncate};

use super::{Filter, Narrowing, OpenSelection, Selection, window};

/// promptの1画面を組み立てる。
pub(super) struct Painter {
    pub(super) catalog: Catalog,
    pub(super) policy: StreamPolicy,
}

impl Painter {
    fn text(&self, id: &'static str) -> String {
        self.format(&msg!(id))
    }

    fn format(&self, message: &Msg) -> String {
        self.catalog
            .format(message)
            .unwrap_or_else(|failure| failure.to_string())
    }

    fn paint(&self, text: &str, role: Role) -> String {
        if !self.policy.color {
            return text.to_string();
        }
        crate::design::paint(text, style::role_style(role))
    }

    pub(crate) fn heading(&self, message: &Msg) -> String {
        self.paint(&self.format(message), Role::Heading)
    }

    fn muted(&self, text: &str) -> String {
        self.paint(text, Role::Muted)
    }

    fn glyphs(&self) -> GlyphSet {
        style::glyphs(self.policy.characters)
    }

    /// 確定した値を一行の結果として残す。
    pub(crate) fn selected(&self, value: &str) -> String {
        format!(
            "{} {}",
            self.paint(self.glyphs().success, Role::SuccessMarker),
            self.format(&msg!("prompt-selected", value = value))
        )
    }

    /// `open`で確定した案件とworktree indexを一行の結果として残す。
    pub(crate) fn selected_open(&self, project: &str, index: u32) -> String {
        format!(
            "{} {}",
            self.paint(self.glyphs().success, Role::SuccessMarker),
            self.format(&msg!(
                "prompt-selected-open",
                project = project,
                index = index
            ))
        )
    }

    /// 使えるkeyと動作を必ず対で示す。key名は翻訳せず、動作だけを訳す。
    pub(crate) fn keys(&self, multi: bool, filter: Filter) -> String {
        let glyphs = self.glyphs();
        let mut pairs = vec![format!(
            "{}/{} {}",
            glyphs.arrow_up,
            glyphs.arrow_down,
            self.text("prompt-key-move")
        )];
        if multi {
            pairs.push(format!("Space {}", self.text("prompt-key-toggle")));
        }
        if filter == Filter::Typed {
            pairs.push(self.filter_key());
        }
        pairs.push(format!("Enter {}", self.text("prompt-key-confirm")));
        pairs.push(format!("Esc {}", self.text("prompt-key-cancel")));
        pairs.join("   ")
    }

    /// 文字キーは1つの名前を持たないため、打つことそのものをkey名の位置に置く。
    fn filter_key(&self) -> String {
        format!("Type {}", self.text("prompt-key-filter"))
    }

    /// `open`の案件とworktree index promptで使えるkeyと動作を対で示す。
    fn open_keys(&self) -> String {
        let glyphs = self.glyphs();
        [
            format!(
                "{}/{} {}",
                glyphs.arrow_up,
                glyphs.arrow_down,
                self.text("prompt-key-move-project")
            ),
            format!(
                "{}/{} {}",
                glyphs.arrow_left,
                glyphs.arrow_right,
                self.text("prompt-key-adjust-index")
            ),
            self.filter_key(),
            format!("Enter {}", self.text("prompt-key-confirm")),
            format!("Esc {}", self.text("prompt-key-cancel")),
        ]
        .join("   ")
    }

    /// 絞り込みの一行。打った文字と、見えている候補の数を示す。
    ///
    /// 何も打っていなければ、打てば狭まることをmutedで示す。打った文字は利用者の値であり、
    /// 既定色で出す。
    fn filter_line(&self, narrowing: &Narrowing) -> String {
        let label = self.text("prompt-filter-label");
        if !narrowing.has_query() {
            return format!(
                "  {} {}",
                self.muted(&label),
                self.muted(&self.text("prompt-filter-hint"))
            );
        }
        let count = self.format(&msg!(
            "prompt-filter-count",
            shown = narrowing.matches().len(),
            total = narrowing.total()
        ));
        // 幅を超える場合も数は残し、打った文字の末尾だけを省く。
        let query = match self.policy.width {
            Some(width) => {
                let fixed = 2 + display_width(&label) + 1 + 2 + display_width(&count);
                truncate(narrowing.query(), width.saturating_sub(fixed).max(4))
            }
            None => narrowing.query().to_string(),
        };
        format!("  {} {query}  {}", self.muted(&label), self.muted(&count))
    }

    /// 一致する候補が無いまま確定しようとしたことを、一覧の直上へ出す。
    fn no_match_warning(&self, narrowing: &Narrowing) -> String {
        let glyphs = self.glyphs();
        format!(
            "{} {} {}",
            self.paint(glyphs.warning, Role::WarningMarker),
            self.paint(&self.text("warning-label"), Role::WarningMarker),
            self.format(&msg!("prompt-filter-no-match", query = narrowing.query()))
        )
    }

    /// `open`の案件とworktree indexを同時に選ぶ1画面を組み立てる。
    ///
    /// 候補以外の行数は[`super::OPEN_FRAME_FIXED_ROWS`]と一致させる。
    pub(crate) fn open_frame(
        &self,
        heading: &Msg,
        labels: &[String],
        selection: &OpenSelection,
        viewport: Option<usize>,
    ) -> Vec<String> {
        let current_label = format!("({})", self.text("prompt-current"));
        let narrowing = selection.narrowing();
        let mut lines = vec![self.heading(heading), String::new()];
        lines.push(format!("  {}", self.muted(&self.open_keys())));
        lines.push(String::new());
        lines.push(self.filter_line(narrowing));
        // metadataを読めなかった案件は範囲を述べない。数を示すと、読めていない値を
        // 読めたかのように見せることになる。
        let index = match selection.maximum_index() {
            Some(maximum) => msg!(
                "prompt-worktree-index",
                index = selection.current_index(),
                maximum = maximum
            ),
            None => msg!(
                "prompt-worktree-index-unreadable",
                index = selection.current_index()
            ),
        };
        lines.push(format!("  {}", self.format(&index)));
        if selection.warns_about_no_match() {
            lines.push(self.no_match_warning(narrowing));
        }
        lines.push(String::new());

        let visible = selection.visible();
        let position = visible
            .iter()
            .position(|index| *index == selection.current_project())
            .unwrap_or(0);
        for slot in window(visible.len(), position, viewport) {
            let index = visible[slot];
            lines.push(self.candidate(
                &labels[index],
                index == selection.current_project(),
                None,
                &current_label,
                narrowing.match_range(index),
            ));
        }
        lines
    }

    pub(crate) fn frame(
        &self,
        heading: &Msg,
        labels: &[String],
        selection: &Selection,
        viewport: Option<usize>,
    ) -> Vec<String> {
        let glyphs = self.glyphs();
        let narrowing = selection.narrowing();
        let mut lines = vec![self.heading(heading), String::new()];

        lines.push(format!(
            "  {}",
            self.muted(&self.keys(selection.is_multi(), narrowing.filter()))
        ));
        if narrowing.is_typed() {
            lines.push(self.filter_line(narrowing));
        }
        if selection.is_multi() {
            // zeroも省かない。画面外の候補を選んでいることを常に見せる。絞り込みで隠れた
            // 選択も、隠れたまま確定することが分かるように数える。
            let hidden = selection.hidden_checked_count();
            let count = if hidden > 0 {
                msg!(
                    "prompt-selected-count-hidden",
                    count = selection.selected_count(),
                    hidden = hidden
                )
            } else {
                msg!("prompt-selected-count", count = selection.selected_count())
            };
            lines.push(format!("  {}", self.muted(&self.format(&count))));
        }
        if selection.warns_about_empty() {
            lines.push(format!(
                "{} {} {}",
                self.paint(glyphs.warning, Role::WarningMarker),
                self.paint(&self.text("warning-label"), Role::WarningMarker),
                self.text("prompt-select-at-least-one")
            ));
        }
        if selection.warns_about_no_match() {
            lines.push(self.no_match_warning(narrowing));
        }
        lines.push(String::new());

        let current_label = format!("({})", self.text("prompt-current"));
        let visible = selection.visible();
        let position = visible
            .iter()
            .position(|index| *index == selection.current())
            .unwrap_or(0);
        for slot in window(visible.len(), position, viewport) {
            let index = visible[slot];
            lines.push(self.candidate(
                &labels[index],
                index == selection.current(),
                selection.is_multi().then(|| selection.is_checked(index)),
                &current_label,
                narrowing.match_range(index),
            ));
        }
        lines
    }

    fn candidate(
        &self,
        label: &str,
        current: bool,
        checked: Option<bool>,
        current_label: &str,
        matched: Option<Range<usize>>,
    ) -> String {
        // `›`はfocusだけ、`[x]`は選択済みだけを表す。両方の状態を同時に見せる。
        let marker = if current {
            self.paint(self.glyphs().current, Role::PromptCurrent)
        } else {
            " ".to_string()
        };
        let checkbox = match checked {
            Some(true) => format!("{} ", self.paint("[x]", Role::PromptChecked)),
            Some(false) => "[ ] ".to_string(),
            None => String::new(),
        };
        let suffix = if current {
            format!("  {}", self.muted(current_label))
        } else {
            String::new()
        };

        // 幅を超える場合もmarkerとstateを残し、labelの末尾だけを省く。横折り返しで
        // 次の候補に見えてしまうのを避ける。
        let fixed = 2
            + checked.map_or(0, |_| 4)
            + if current {
                display_width(current_label) + 2
            } else {
                0
            };
        let (kept, ellipsis) = match self.policy.width {
            Some(width) => shorten(label, width.saturating_sub(fixed).max(4)),
            None => (label, ""),
        };
        // 一致した範囲は、見えている部分だけを強調する。省略記号は一致ではない。
        let matched = matched
            .map(|range| range.start.min(kept.len())..range.end.min(kept.len()))
            .filter(|range| range.start < range.end);
        let label = match matched {
            Some(range) => format!(
                "{}{}{}",
                self.label(&kept[..range.start], current, false),
                self.label(&kept[range.clone()], current, true),
                self.label(&kept[range.end..], current, false)
            ),
            None => self.label(kept, current, false),
        };
        let ellipsis = self.label(ellipsis, current, false);
        format!("{marker} {checkbox}{label}{ellipsis}{suffix}")
    }

    /// labelの一部。現在位置の行は全体を前へ出し、絞り込みに一致した部分へunderlineを重ねる。
    fn label(&self, text: &str, current: bool, matched: bool) -> String {
        match (current, matched) {
            (true, true) => self.paint(text, Role::PromptCurrentMatch),
            (true, false) => self.paint(text, Role::PromptCurrent),
            (false, true) => self.paint(text, Role::PromptMatch),
            (false, false) => text.to_string(),
        }
    }
}
