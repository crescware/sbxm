use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{Error, ErrorId, Msg, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use crate::design::Trace;
use crate::design::policy::StreamPolicy;
use crate::design::width::display_width;

use super::{
    Key, Keys, OpenSelection, Painter, Screen, Selection, Transition, action_for, open_viewport,
    unreadable, viewport,
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
    /// `--verbose`の実行が、答えを待った時間を並べる先。`Ui`と共有する。
    trace: Option<Rc<RefCell<Trace>>>,
}

impl PromptUi {
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
            trace: None,
        }
    }

    /// 答えを待った時間を`trace`へも知らせるようにする。
    pub fn attach_trace(&mut self, trace: Rc<RefCell<Trace>>) {
        self.trace = Some(trace);
    }

    /// `ask`が利用者の答えを待つあいだを、traceへ知らせる。
    ///
    /// 待っている時間はsbxmの処理ではない。工程や外部commandに混ぜず、利用者が
    /// 持っていた時間として数えさせる。答えが取り消しや失敗でも、待った時間は同じ。
    /// 候補が無く訊かずに終わる場合は、待っていないため何も知らせない。
    fn waiting<T>(
        &mut self,
        heading: &Msg,
        ask: impl FnOnce(&mut PromptUi) -> Result<T>,
    ) -> Result<T> {
        let started = self
            .trace
            .as_ref()
            .map(|trace| trace.borrow_mut().prompt_started());
        let answer = ask(self);
        if let (Some(trace), Some(started)) = (&self.trace, started) {
            trace.borrow_mut().prompt_answered(heading.id, started);
        }
        answer
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

    /// `open`の案件とworktree indexを上下・左右キーで同時に選ぶ。
    ///
    /// `maximums`は案件ごとの最後のindex。promptを開く前に揃っており、描画のあいだに
    /// 変わらない。`None`の案件は範囲を数として見せず、indexを0から動かさない。
    pub fn select_open(
        &mut self,
        heading: &Msg,
        labels: &[String],
        maximums: &[Option<u32>],
    ) -> Result<(usize, u32)> {
        if labels.is_empty() {
            return Err(unresolved(0, 0));
        }
        self.waiting(heading, |prompt| prompt.ask_open(heading, labels, maximums))
    }

    fn ask_open(
        &mut self,
        heading: &Msg,
        labels: &[String],
        maximums: &[Option<u32>],
    ) -> Result<(usize, u32)> {
        let mut selection = OpenSelection::new(labels.len(), maximums);

        let _ = self.screen.hide_cursor();
        let mut drawn = 0usize;
        let outcome = loop {
            let frame = self.painter.open_frame(
                heading,
                labels,
                &selection,
                open_viewport(self.screen.rows()),
            );
            if let Err(error) = redraw(self.screen.as_mut(), drawn, &frame) {
                break Err(unreadable(&error));
            }
            drawn = frame.len();

            match self.keys.read_key() {
                Ok(key) => match selection.apply(action_for(key)) {
                    Transition::Continue => {}
                    Transition::DoneOpen { project, index } => break Ok((project, index)),
                    Transition::Done(_) => {
                        break Err(unresolved(0, labels.len()));
                    }
                    Transition::Canceled => break Err(Error::Canceled),
                },
                Err(error) => break Err(unreadable(&error)),
            }
        };
        let _ = self.screen.show_cursor();
        let _ = self.screen.clear_last_lines(drawn);

        let (project, index) = outcome?;
        let selected = self.painter.selected_open(&labels[project], index);
        let _ = self.screen.write_line(&selected);
        Ok((project, index))
    }

    /// 候補から1件以上を選ぶ。未選択の確定は受け付けない。
    pub fn select_many(&mut self, heading: &Msg, labels: &[String]) -> Result<Vec<usize>> {
        self.select(heading, labels, true)
    }

    fn select(&mut self, heading: &Msg, labels: &[String], multi: bool) -> Result<Vec<usize>> {
        if labels.is_empty() {
            return Err(unresolved(0, 0));
        }
        self.waiting(heading, |prompt| prompt.ask(heading, labels, multi))
    }

    fn ask(&mut self, heading: &Msg, labels: &[String], multi: bool) -> Result<Vec<usize>> {
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
                Ok(key) => match selection.apply(action_for(key)) {
                    Transition::Continue => {}
                    Transition::Done(indexes) => break Ok(indexes),
                    Transition::DoneOpen { .. } => {
                        break Err(unresolved(0, labels.len()));
                    }
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
        self.waiting(heading, |prompt| prompt.ask_line(heading, initial))
    }

    fn ask_line(&mut self, heading: &Msg, initial: &str) -> Result<String> {
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
