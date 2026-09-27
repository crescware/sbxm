use super::{Action, Transition};

/// `open`で案件とmanaged worktree indexを同時に選ぶ状態。
///
/// 上下キーは案件、左右キーはindexへ割り当てる。案件ごとの最後のindexは組み立てる
/// 時点で揃っている。metadataを読めなかった案件は`None`で、範囲を持たずindexは0から
/// 動かない。案件を移ると、indexは移った先の範囲へ収める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSelection {
    project_count: usize,
    current_project: usize,
    current_index: u32,
    maximum_indexes: Vec<Option<u32>>,
}

impl OpenSelection {
    /// `maximum_indexes`が`project_count`に足りない案件は、metadataを読めなかった案件と
    /// 同じく範囲を持たない。
    pub fn new(project_count: usize, maximum_indexes: &[Option<u32>]) -> OpenSelection {
        OpenSelection {
            project_count,
            current_project: 0,
            current_index: 0,
            maximum_indexes: maximum_indexes.to_vec(),
        }
    }

    pub fn current_project(&self) -> usize {
        self.current_project
    }

    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    /// 現在の案件の最後のindex。metadataを読めなかった案件は`None`。
    pub fn maximum_index(&self) -> Option<u32> {
        self.maximum_indexes
            .get(self.current_project)
            .copied()
            .flatten()
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
                self.current_index = self.current_index.saturating_add(1).min(self.bound());
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

    /// いま動かせるindexの上限。範囲を持たない案件では0から動かさない。
    fn bound(&self) -> u32 {
        self.maximum_index().unwrap_or(0)
    }

    fn move_project(&mut self, offset: usize) {
        if self.project_count == 0 {
            return;
        }
        self.current_project = (self.current_project + offset) % self.project_count;
        self.current_index = self.current_index.min(self.bound());
    }
}

#[cfg(test)]
#[path = "open_selection_test.rs"]
mod open_selection_test;
