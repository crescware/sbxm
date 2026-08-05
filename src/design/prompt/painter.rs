use crate::diagnostics::Msg;
use crate::i18n::Catalog;
use crate::msg;

use crate::design::policy::StreamPolicy;
use crate::design::style::{self, GlyphSet, Role};
use crate::design::width::{display_width, truncate};

use super::{OpenSelection, Selection, window};

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
    pub(crate) fn keys(&self, multi: bool) -> String {
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
        pairs.push(format!("Enter {}", self.text("prompt-key-confirm")));
        pairs.push(format!("Esc {}", self.text("prompt-key-cancel")));
        pairs.join("   ")
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
            format!("Enter {}", self.text("prompt-key-confirm")),
            format!("Esc {}", self.text("prompt-key-cancel")),
        ]
        .join("   ")
    }

    /// `open`の案件とworktree indexを同時に選ぶ1画面を組み立てる。
    pub(crate) fn open_frame(
        &self,
        heading: &Msg,
        labels: &[String],
        selection: &OpenSelection,
        viewport: Option<usize>,
    ) -> Vec<String> {
        let current_label = format!("({})", self.text("prompt-current"));
        let mut lines = vec![self.heading(heading), String::new()];
        lines.push(format!("  {}", self.muted(&self.open_keys())));
        lines.push(String::new());
        lines.push(format!(
            "  {}",
            self.format(&msg!(
                "prompt-worktree-index",
                index = selection.current_index(),
                maximum = selection.maximum_index()
            ))
        ));
        lines.push(String::new());

        for index in window(labels.len(), selection.current_project(), viewport) {
            lines.push(self.candidate(
                &labels[index],
                index == selection.current_project(),
                None,
                &current_label,
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
        let mut lines = vec![self.heading(heading), String::new()];

        lines.push(format!(
            "  {}",
            self.muted(&self.keys(selection.is_multi()))
        ));
        if selection.is_multi() {
            // zeroも省かない。画面外の候補を選んでいることを常に見せる。
            lines.push(format!(
                "  {}",
                self.muted(&self.format(&msg!(
                    "prompt-selected-count",
                    count = selection.selected_count()
                )))
            ));
        }
        if selection.warns_about_empty() {
            lines.push(format!(
                "{} {} {}",
                self.paint(glyphs.warning, Role::WarningMarker),
                self.paint(&self.text("warning-label"), Role::WarningMarker),
                self.text("prompt-select-at-least-one")
            ));
        }
        lines.push(String::new());

        let current_label = format!("({})", self.text("prompt-current"));
        for index in window(labels.len(), selection.current(), viewport) {
            lines.push(self.candidate(
                &labels[index],
                index == selection.current(),
                selection.is_multi().then(|| selection.is_checked(index)),
                &current_label,
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
        let label = match self.policy.width {
            Some(width) => truncate(label, width.saturating_sub(fixed).max(4)),
            None => label.to_string(),
        };
        let label = if current {
            self.paint(&label, Role::PromptCurrent)
        } else {
            label
        };
        format!("{marker} {checkbox}{label}{suffix}")
    }
}
