use super::{Action, Filter, Narrowing, Refusal, Transition};

/// 選択promptの状態。端末を持たない。
///
/// focusとchecked stateを別のfieldで持つのは、現在位置かつ選択済みという状態を
/// 失わないためである。どちらも行ではなく候補に付くため、絞り込みで行が隠れても失わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    narrowing: Narrowing,
    current: usize,
    checked: Vec<bool>,
    multi: bool,
    /// 未選択の確定を受け付けないか。
    require_one: bool,
    /// 直前の確定を受け付けなかった理由。
    refusal: Option<Refusal>,
}

impl Selection {
    /// 候補を決めて開始する。
    ///
    /// 初期状態で暗黙に一件を選択済みにしない。単一選択でも、Enterまでは未確定である。
    pub fn new(labels: &[String], multi: bool, require_one: bool, filter: Filter) -> Selection {
        Selection {
            narrowing: Narrowing::new(labels, filter),
            current: 0,
            checked: vec![false; labels.len()],
            multi,
            require_one,
            refusal: None,
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

    /// 選択済みのうち、絞り込みで隠れている件数。
    ///
    /// 隠れた候補もEnterで一緒に確定する。見えないまま確定することが分かるよう、選択数の
    /// 行に添える。
    pub fn hidden_checked_count(&self) -> usize {
        self.checked
            .iter()
            .enumerate()
            .filter(|(index, checked)| **checked && !self.narrowing.is_match(*index))
            .count()
    }

    /// 未選択のまま確定しようとしたか。
    pub fn warns_about_empty(&self) -> bool {
        self.refusal == Some(Refusal::Empty)
    }

    /// 一致する候補が無いまま確定しようとしたか。
    pub fn warns_about_no_match(&self) -> bool {
        self.refusal == Some(Refusal::NoMatch)
    }

    pub fn narrowing(&self) -> &Narrowing {
        &self.narrowing
    }

    /// 見えている候補のindex。並び順は候補のまま。
    pub fn visible(&self) -> Vec<usize> {
        self.narrowing.matches()
    }

    /// 打鍵を状態へ反映する。
    pub fn apply(&mut self, action: Action) -> Transition {
        match action {
            Action::Previous => {
                self.current = self.narrowing.previous(self.current);
                Transition::Continue
            }
            Action::Next => {
                self.current = self.narrowing.next(self.current);
                Transition::Continue
            }
            Action::Toggle => {
                // 隠れている候補は選び直せない。見えている現在位置だけを切り替える。
                if self.multi
                    && self.narrowing.is_match(self.current)
                    && let Some(checked) = self.checked.get_mut(self.current)
                {
                    *checked = !*checked;
                    // 選択したのだから、直前の警告は役目を終える。
                    self.refusal = None;
                }
                Transition::Continue
            }
            Action::Confirm => self.confirm(),
            Action::Cancel => Transition::Canceled,
            Action::Type(character) => {
                if self.narrowing.type_char(character) {
                    self.narrowed();
                }
                Transition::Continue
            }
            Action::Erase => {
                if self.narrowing.erase() {
                    self.narrowed();
                }
                Transition::Continue
            }
            Action::DecreaseIndex | Action::IncreaseIndex | Action::Ignore => Transition::Continue,
        }
    }

    /// 絞り込みが変わった。現在位置を見える候補へ置き直し、一致が無かった警告は役目を終える。
    fn narrowed(&mut self) {
        self.current = self.narrowing.anchored(self.current);
        if self.refusal == Some(Refusal::NoMatch) {
            self.refusal = None;
        }
    }

    fn confirm(&mut self) -> Transition {
        // 一致する候補が無いまま確定しても、隠れた候補を選んだことにはしない。
        if self.narrowing.has_query() && self.narrowing.matches().is_empty() {
            self.refusal = Some(Refusal::NoMatch);
            return Transition::Continue;
        }
        if !self.multi {
            return Transition::Done(vec![self.current]);
        }
        let selected: Vec<usize> = (0..self.checked.len())
            .filter(|index| self.is_checked(*index))
            .collect();
        if selected.is_empty() && self.require_one {
            // 説明なく同じpromptを描き直さない。現在位置も動かさない。
            self.refusal = Some(Refusal::Empty);
            return Transition::Continue;
        }
        Transition::Done(selected)
    }
}
