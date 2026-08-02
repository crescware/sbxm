use console::{Key, Term};

use crate::diagnostics::{Error, ErrorId, Msg, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use crate::design::policy::StreamPolicy;
use crate::design::width::display_width;

use super::{
    Keys, Painter, RealTerminal, Screen, Selection, Transition, action_for, unreadable, viewport,
};

/// 対話の入口。
///
/// project選択と`init`の言語選択が別々のthemeを持たないよう、すべてここを通す。
///
/// 打鍵の供給元と描画先は受け取る。実端末を選ぶのは組み立てる側であり、選択と入力の
/// 進み方はここだけで決まる。
pub struct PromptUi {
    painter: Painter,
    keys: Box<dyn Keys>,
    screen: Box<dyn Screen>,
}

impl PromptUi {
    /// 実端末へ出すprompt。
    ///
    /// 出す先はstderrとする。正常結果はstdoutへ残るため、描き直しと混ざらない。
    pub fn terminal(locale: Locale, policy: StreamPolicy) -> PromptUi {
        let term = Term::stderr();
        PromptUi::new(
            locale,
            policy,
            Box::new(RealTerminal::new(term.clone())),
            Box::new(RealTerminal::new(term)),
        )
    }

    /// 打鍵の供給元と描画先を決めて組み立てる。
    pub fn new(
        locale: Locale,
        policy: StreamPolicy,
        keys: Box<dyn Keys>,
        screen: Box<dyn Screen>,
    ) -> PromptUi {
        PromptUi {
            painter: Painter {
                catalog: Catalog::new(locale),
                policy,
            },
            keys,
            screen,
        }
    }

    /// 表示言語を差し替える。
    ///
    /// 訊く言語は結果を書く言語と同じでなければならない。`Ui::set_locale`と同じ時点で
    /// 呼ぶ。
    pub fn set_locale(&mut self, locale: Locale) {
        if self.painter.catalog.locale() != locale {
            self.painter.catalog = Catalog::new(locale);
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
        let mut selection = Selection::new(labels.len(), multi, true);

        let _ = self.screen.hide_cursor();
        let mut drawn = 0usize;
        let outcome = loop {
            let frame =
                self.painter
                    .frame(heading, labels, &selection, viewport(self.screen.rows()));
            if let Err(error) = redraw(self.screen.as_mut(), drawn, &frame) {
                break Err(unreadable(&error));
            }
            drawn = frame.len();

            match self.keys.read_key() {
                Ok(key) => match selection.apply(action_for(&key)) {
                    Transition::Continue => {}
                    Transition::Done(indexes) => break Ok(indexes),
                    Transition::Canceled => break Err(Error::Canceled),
                },
                Err(error) => break Err(unreadable(&error)),
            }
        };
        let _ = self.screen.show_cursor();
        let _ = self.screen.clear_last_lines(drawn);

        let indexes = outcome?;
        // 選んだ値は一行の結果として残す。promptと答えを一行へ潰さない。
        let chosen: Vec<&str> = indexes
            .iter()
            .filter_map(|index| labels.get(*index).map(String::as_str))
            .collect();
        let selected = self.painter.selected(&chosen.join(", "));
        let _ = self.screen.write_line(&selected);
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
        let heading = self.painter.heading(heading);
        self.screen
            .write_line(&heading)
            .map_err(|error| unreadable(&error))?;

        let mut typed = String::from(initial);
        if !typed.is_empty() {
            self.screen
                .write_str(&typed)
                .map_err(|error| unreadable(&error))?;
        }
        loop {
            match self.keys.read_key().map_err(|error| unreadable(&error))? {
                Key::Enter => break,
                Key::Escape | Key::CtrlC => return Err(Error::Canceled),
                Key::Backspace => {
                    if let Some(removed) = typed.pop() {
                        self.screen
                            .clear_chars(display_width(&removed.to_string()))
                            .map_err(|error| unreadable(&error))?;
                    }
                }
                Key::Char(character) => {
                    typed.push(character);
                    self.screen
                        .write_str(&character.to_string())
                        .map_err(|error| unreadable(&error))?;
                }
                // 行編集は提供しない。入力に必要な打鍵だけを受け取る。
                _ => {}
            }
        }
        self.screen
            .write_line("")
            .map_err(|error| unreadable(&error))?;
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

/// 前回描いた行を消してから描き直す。
fn redraw(screen: &mut dyn Screen, drawn: usize, frame: &[String]) -> std::io::Result<()> {
    if drawn > 0 {
        screen.clear_last_lines(drawn)?;
    }
    for line in frame {
        screen.write_line(line)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "prompt_ui_test.rs"]
mod prompt_ui_test;
