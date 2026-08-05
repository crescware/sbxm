use super::{Action, Transition};

/// `open`のworktree indexを選ぶ状態。
///
/// 案件ごとに異なる最大値を受け取り、左右キーでその範囲から出ないようにする。
/// project選択の`Selection`とは別の状態にして、左右キーがほかの選択promptへ
/// 意図せず意味を持たないようにする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexSelection {
    current: u32,
    maximum: u32,
}

impl IndexSelection {
    /// index 0から始める。worktree数が1件ならmaximumも0になる。
    pub fn new(maximum: u32) -> IndexSelection {
        IndexSelection {
            current: 0,
            maximum,
        }
    }

    pub fn current(self) -> u32 {
        self.current
    }

    pub fn maximum(self) -> u32 {
        self.maximum
    }

    /// 打鍵をindexの状態へ反映する。上下キーなどindexに関係しない操作は無視する。
    pub fn apply(&mut self, action: Action) -> Transition {
        match action {
            Action::DecreaseIndex => {
                self.current = self.current.saturating_sub(1);
                Transition::Continue
            }
            Action::IncreaseIndex => {
                self.current = self.current.saturating_add(1).min(self.maximum);
                Transition::Continue
            }
            Action::Confirm => Transition::DoneIndex(self.current),
            Action::Cancel => Transition::Canceled,
            Action::Previous | Action::Next | Action::Toggle | Action::Ignore => {
                Transition::Continue
            }
        }
    }
}

#[cfg(test)]
#[path = "index_selection_test.rs"]
mod index_selection_test;
