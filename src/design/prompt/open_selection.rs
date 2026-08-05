use super::{Action, Transition};

/// `open`で案件とmanaged worktree indexを同時に選ぶ状態。
///
/// 上下キーは案件、左右キーはindexへ割り当てる。metadataはpromptを表示する前に
/// 読まないため、最大値は呼び出し側が渡す楽観的な値から始める。確定後にlock済み
/// metadataの最大値へclampする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSelection {
    project_count: usize,
    current_project: usize,
    current_index: u32,
    maximum_index: u32,
}

impl OpenSelection {
    pub fn new(project_count: usize, maximum_index: u32) -> OpenSelection {
        OpenSelection {
            project_count,
            current_project: 0,
            current_index: 0,
            maximum_index,
        }
    }

    pub fn current_project(self) -> usize {
        self.current_project
    }

    pub fn current_index(self) -> u32 {
        self.current_index
    }

    pub fn maximum_index(self) -> u32 {
        self.maximum_index
    }

    /// 打鍵を案件またはindexの状態へ反映する。
    pub fn apply(&mut self, action: Action) -> Transition {
        match action {
            Action::Previous => {
                self.move_project(self.project_count.saturating_sub(1));
                Transition::Continue
            }
            Action::Next => {
                self.move_project(1);
                Transition::Continue
            }
            Action::DecreaseIndex => {
                self.current_index = self.current_index.saturating_sub(1);
                Transition::Continue
            }
            Action::IncreaseIndex => {
                self.current_index = self.current_index.saturating_add(1).min(self.maximum_index);
                Transition::Continue
            }
            Action::Confirm => Transition::DoneOpen {
                project: self.current_project,
                index: self.current_index,
            },
            Action::Cancel => Transition::Canceled,
            Action::Toggle | Action::Ignore => Transition::Continue,
        }
    }

    fn move_project(&mut self, offset: usize) {
        if self.project_count == 0 {
            return;
        }
        self.current_project = (self.current_project + offset) % self.project_count;
    }
}

#[cfg(test)]
#[path = "open_selection_test.rs"]
mod open_selection_test;
