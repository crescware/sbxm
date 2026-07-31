//! 選択promptと、自由入力。
//!
//! promptは「候補」「現在位置」「選択済み」を別の状態として描く。cursor一文字だけに
//! 識別を委ねると、色を消したときと画面外へ選択が出たときに状態が読めなくなる。
//!
//! 描画と打鍵の解釈を[`Selection`]と端末adapterへ分ける。状態遷移は端末なしで確かめられ、
//! 端末adapterは描画だけを持つ。
//!
//! prompt libraryの既定themeへ要件を合わせず、単一選択と複数選択で同じ描画を使う。
//! 自由入力とyes/noも同じ入口から出すのは、commandごとにpromptの見た目が変わらない
//! ようにするためである。
//!
//! [`PromptUi`]はlocaleと描画条件を写しで持つ。`Ui`を借りたままにしないことで、
//! 同じworkflowが進捗の報告とpromptの両方を必要としても借用が衝突しない。

use console::{Key, Term};

use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use super::Remediation;
use super::policy::StreamPolicy;
use super::style::{self, GlyphSet, Role};
use super::width::{display_width, truncate};

/// 打鍵が意味する操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Previous,
    Next,
    Toggle,
    Confirm,
    Cancel,
    /// 受け付けない打鍵。状態を変えない。
    Ignore,
}

/// 操作の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// 描き直して続ける。
    Continue,
    /// 確定した候補のindex。
    Done(Vec<usize>),
    /// 何も変更せず終える。
    Canceled,
}

/// 選択promptの状態。端末を持たない。
///
/// focusとchecked stateを別のfieldで持つのは、現在位置かつ選択済みという状態を
/// 失わないためである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    count: usize,
    current: usize,
    checked: Vec<bool>,
    multi: bool,
    /// 未選択の確定を受け付けないか。
    require_one: bool,
    /// 直前の確定が未選択だったか。警告を一覧の直上へ出す。
    empty_confirm: bool,
}

impl Selection {
    /// 候補数を決めて開始する。
    ///
    /// 初期状態で暗黙に一件を選択済みにしない。単一選択でも、Enterまでは未確定である。
    pub fn new(count: usize, multi: bool, require_one: bool) -> Selection {
        Selection {
            count,
            current: 0,
            checked: vec![false; count],
            multi,
            require_one,
            empty_confirm: false,
        }
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn is_multi(&self) -> bool {
        self.multi
    }

    pub fn is_checked(&self, index: usize) -> bool {
        self.checked.get(index).copied().unwrap_or(false)
    }

    /// 選択済みの件数。zeroも表示するため、常に数えられる。
    pub fn selected_count(&self) -> usize {
        self.checked.iter().filter(|checked| **checked).count()
    }

    /// 未選択のまま確定しようとしたか。
    pub fn warns_about_empty(&self) -> bool {
        self.empty_confirm
    }

    /// 打鍵を状態へ反映する。
    pub fn apply(&mut self, action: Action) -> Transition {
        match action {
            Action::Previous => {
                self.move_by(self.count.saturating_sub(1));
                Transition::Continue
            }
            Action::Next => {
                self.move_by(1);
                Transition::Continue
            }
            Action::Toggle => {
                if self.multi && self.current < self.checked.len() {
                    self.checked[self.current] = !self.checked[self.current];
                    // 選択したのだから、直前の警告は役目を終える。
                    self.empty_confirm = false;
                }
                Transition::Continue
            }
            Action::Confirm => self.confirm(),
            Action::Cancel => Transition::Canceled,
            Action::Ignore => Transition::Continue,
        }
    }

    fn confirm(&mut self) -> Transition {
        if !self.multi {
            return Transition::Done(vec![self.current]);
        }
        let selected: Vec<usize> = (0..self.count)
            .filter(|index| self.checked[*index])
            .collect();
        if selected.is_empty() && self.require_one {
            // 説明なく同じpromptを描き直さない。現在位置も動かさない。
            self.empty_confirm = true;
            return Transition::Continue;
        }
        Transition::Done(selected)
    }

    fn move_by(&mut self, offset: usize) {
        if self.count == 0 {
            return;
        }
        self.current = (self.current + offset) % self.count;
    }
}

/// 打鍵から操作を決める。
///
/// 行編集は提供しない。選択に必要な打鍵だけを受け取る。
pub fn action_for(key: &Key) -> Action {
    match key {
        Key::ArrowUp => Action::Previous,
        Key::ArrowDown => Action::Next,
        Key::Char(' ') => Action::Toggle,
        Key::Enter => Action::Confirm,
        Key::Escape | Key::CtrlC => Action::Cancel,
        _ => Action::Ignore,
    }
}

/// 端末が読めなかったことを報告する。
///
/// 中断は何も変更せず終える。それ以外を引数の不足として報告しない。
pub fn unreadable(error: &std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::Interrupted {
        return Error::Canceled;
    }
    Error::single(
        Diagnostic::new(
            ErrorId::PromptUnreadable,
            msg!("error-prompt-unreadable", detail = error),
        )
        .remediation(Remediation::text(msg!("remediation-prompt-unreadable"))),
    )
}

/// 対話の入口。
///
/// project選択と`init`の言語選択が別々のthemeを持たないよう、すべてここを通す。
pub struct PromptUi {
    painter: Painter,
}

impl PromptUi {
    pub fn new(locale: Locale, policy: StreamPolicy) -> PromptUi {
        PromptUi {
            painter: Painter {
                catalog: Catalog::new(locale),
                policy,
            },
        }
    }

    /// 候補から1件を選ぶ。
    pub fn select_one(&mut self, heading: &Msg, labels: &[String]) -> Result<usize> {
        let selected = self.select(heading, labels, false)?;
        selected
            .first()
            .copied()
            .ok_or_else(|| unresolved(0, labels.len()))
    }

    /// 候補から1件以上を選ぶ。未選択の確定は受け付けない。
    pub fn select_many(&mut self, heading: &Msg, labels: &[String]) -> Result<Vec<usize>> {
        self.select(heading, labels, true)
    }

    fn select(&mut self, heading: &Msg, labels: &[String], multi: bool) -> Result<Vec<usize>> {
        if labels.is_empty() {
            return Err(unresolved(0, 0));
        }
        let term = Term::stderr();
        let mut selection = Selection::new(labels.len(), multi, true);

        let _ = term.hide_cursor();
        let mut drawn = 0usize;
        let outcome = loop {
            let frame = self
                .painter
                .frame(heading, labels, &selection, viewport(&term));
            if let Err(error) = redraw(&term, drawn, &frame) {
                break Err(unreadable(&error));
            }
            drawn = frame.len();

            match term.read_key() {
                Ok(key) => match selection.apply(action_for(&key)) {
                    Transition::Continue => {}
                    Transition::Done(indexes) => break Ok(indexes),
                    Transition::Canceled => break Err(Error::Canceled),
                },
                Err(error) => break Err(unreadable(&error)),
            }
        };
        let _ = term.show_cursor();
        let _ = term.clear_last_lines(drawn);

        let indexes = outcome?;
        // 選んだ値は一行の結果として残す。promptと答えを一行へ潰さない。
        let chosen: Vec<&str> = indexes
            .iter()
            .filter_map(|index| labels.get(*index).map(String::as_str))
            .collect();
        let _ = term.write_line(&self.painter.selected(&chosen.join(", ")));
        Ok(indexes)
    }

    /// 完全一致だけを続行の合図とする入力。
    pub fn exact(&mut self, heading: &Msg) -> Result<String> {
        self.read_line(heading, "")
    }

    /// 候補を初期値として置いた入力。
    ///
    /// 候補は打ち直せる文字列として現れ、確定した値ではない。空の候補は空欄で始まる。
    pub fn input(&mut self, heading: &Msg, candidate: &str) -> Result<String> {
        self.read_line(heading, candidate)
    }

    fn read_line(&mut self, heading: &Msg, initial: &str) -> Result<String> {
        let term = Term::stderr();
        term.write_line(&self.painter.heading(heading))
            .map_err(|error| unreadable(&error))?;

        let mut typed = String::from(initial);
        if !typed.is_empty() {
            term.write_str(&typed).map_err(|error| unreadable(&error))?;
        }
        loop {
            match term.read_key().map_err(|error| unreadable(&error))? {
                Key::Enter => break,
                Key::Escape | Key::CtrlC => return Err(Error::Canceled),
                Key::Backspace => {
                    if let Some(removed) = typed.pop() {
                        term.clear_chars(display_width(&removed.to_string()))
                            .map_err(|error| unreadable(&error))?;
                    }
                }
                Key::Char(character) => {
                    typed.push(character);
                    term.write_str(&character.to_string())
                        .map_err(|error| unreadable(&error))?;
                }
                // 行編集は提供しない。入力に必要な打鍵だけを受け取る。
                _ => {}
            }
        }
        term.write_line("").map_err(|error| unreadable(&error))?;
        Ok(typed)
    }
}

/// promptが候補に対応しない選択を返した場合。cancelとは区別する。
fn unresolved(index: usize, count: usize) -> Error {
    Error::new(
        ErrorId::SelectionUnresolved,
        msg!("error-selection-unresolved", index = index, count = count),
    )
}

/// 一覧に使える行数。端末の高さを読めない場合は制限しない。
fn viewport(term: &Term) -> Option<usize> {
    if !term.is_term() {
        return None;
    }
    let (rows, _) = term.size();
    // heading、操作説明、選択数、空行、結果の一行ぶんを残す。
    Some((rows as usize).saturating_sub(6).max(1))
}

/// 前回描いた行を消してから描き直す。
fn redraw(term: &Term, drawn: usize, frame: &[String]) -> std::io::Result<()> {
    if drawn > 0 {
        term.clear_last_lines(drawn)?;
    }
    for line in frame {
        term.write_line(line)?;
    }
    Ok(())
}

/// 現在位置が見える範囲の候補index。
///
/// 端末の高さを超える候補があっても、markerと状態を残したまま窓だけを動かす。
pub fn window(count: usize, current: usize, viewport: Option<usize>) -> std::ops::Range<usize> {
    let Some(visible) = viewport.filter(|visible| *visible < count) else {
        return 0..count;
    };
    let start = current.saturating_sub(visible / 2).min(count - visible);
    start..start + visible
}

/// promptの1画面を組み立てる。
struct Painter {
    catalog: Catalog,
    policy: StreamPolicy,
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
        super::renderer::paint(text, style::role_style(role))
    }

    fn heading(&self, message: &Msg) -> String {
        self.paint(&self.format(message), Role::Heading)
    }

    fn muted(&self, text: &str) -> String {
        self.paint(text, Role::Muted)
    }

    fn glyphs(&self) -> GlyphSet {
        style::glyphs(self.policy.characters)
    }

    /// 確定した値を一行の結果として残す。
    fn selected(&self, value: &str) -> String {
        format!(
            "{} {}",
            self.paint(self.glyphs().success, Role::SuccessMarker),
            self.format(&msg!("prompt-selected", value = value))
        )
    }

    /// 使えるkeyと動作を必ず対で示す。key名は翻訳せず、動作だけを訳す。
    fn keys(&self, multi: bool) -> String {
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

    fn frame(
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

#[cfg(test)]
#[path = "prompt_test.rs"]
mod prompt_test;
