use super::{Action, Transition};

/// `open`で案件とmanaged worktree indexを同時に選ぶ状態。
///
/// 上下キーは案件、左右キーはindexへ割り当てる。metadataはprompt表示を待たせないため、
/// 最大値は呼び出し側が渡す楽観的な値から始め、計算結果が届いた案件だけ更新する。
/// 確定後にもlock済みmetadataの最大値へclampする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSelection {
    project_count: usize,
    current_project: usize,
    current_index: u32,
    maximum_indexes: Vec<u32>,
}

impl OpenSelection {
    pub fn new(project_count: usize, maximum_index: u32) -> OpenSelection {
        OpenSelection {
            project_count,
            current_project: 0,
            current_index: 0,
            maximum_indexes: vec![maximum_index; project_count],
        }
    }

    pub fn current_project(&self) -> usize {
        self.current_project
    }

    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    pub fn maximum_index(&self) -> u32 {
        self.maximum_indexes
            .get(self.current_project)
            .copied()
            .unwrap_or(0)
    }

    /// metadataの計算結果で、指定案件の最大indexを更新する。
    pub fn set_maximum(&mut self, project: usize, maximum: u32) {
        let Some(maximum_index) = self.maximum_indexes.get_mut(project) else {
            return;
        };
        *maximum_index = maximum;
        if self.current_project == project {
            self.current_index = self.current_index.min(maximum);
        }
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
                self.current_index = self
                    .current_index
                    .saturating_add(1)
                    .min(self.maximum_index());
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
        self.current_index = self.current_index.min(self.maximum_index());
    }
}

#[cfg(test)]
#[path = "open_selection_test.rs"]
mod open_selection_test;
