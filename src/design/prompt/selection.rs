use super::{Action, Transition};

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
            Action::DecreaseIndex | Action::IncreaseIndex | Action::Ignore => Transition::Continue,
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
