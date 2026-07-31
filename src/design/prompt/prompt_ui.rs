use console::{Key, Term};

use crate::diagnostics::{Error, ErrorId, Msg, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use crate::design::policy::StreamPolicy;
use crate::design::width::display_width;

use super::{Painter, Selection, Transition, action_for, unreadable, viewport};

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
