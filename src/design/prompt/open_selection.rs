use super::{Action, Filter, Narrowing, Transition};

/// `open`で案件とmanaged worktree indexを同時に選ぶ状態。
///
/// 上下キーは案件、左右キーはindexへ割り当てる。文字キーは案件の一覧を絞り込み、上下キーは
/// 一致した案件の中だけを巡る。案件ごとの最後のindexは組み立てる時点で揃っている。
/// metadataを読めなかった案件は`None`で、範囲を持たずindexは0から動かない。案件を移ると、
/// indexは移った先の範囲へ収める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSelection {
    narrowing: Narrowing,
    current_project: usize,
    current_index: u32,
    maximum_indexes: Vec<Option<u32>>,
    /// 直前の確定が、一致する案件の無い絞り込みの上だったか。警告を一覧の直上へ出す。
    no_match_confirm: bool,
}

impl OpenSelection {
    /// 案件の一覧は常に文字キーで絞り込める。
    ///
    /// `maximum_indexes`が`labels`に足りない案件は、metadataを読めなかった案件と
    /// 同じく範囲を持たない。
    pub fn new(labels: &[String], maximum_indexes: &[Option<u32>]) -> OpenSelection {
        OpenSelection {
            narrowing: Narrowing::new(labels, Filter::Typed),
            current_project: 0,
            current_index: 0,
            maximum_indexes: maximum_indexes.to_vec(),
            no_match_confirm: false,
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

    /// 一致する案件が無いまま確定しようとしたか。
    pub fn warns_about_no_match(&self) -> bool {
        self.no_match_confirm
    }

    pub fn narrowing(&self) -> &Narrowing {
        &self.narrowing
    }

    /// 見えている案件のindex。並び順は候補のまま。
    pub fn visible(&self) -> Vec<usize> {
        self.narrowing.matches()
    }

    /// 打鍵を案件またはindexの状態へ反映する。
    pub fn apply(&mut self, action: Action) -> Transition {
        match action {
            Action::Previous => {
                self.move_to(self.narrowing.previous(self.current_project));
                Transition::Continue
            }
            Action::Next => {
                self.move_to(self.narrowing.next(self.current_project));
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
            Action::Confirm => {
                // 一致する案件が無いまま確定しても、隠れた案件を選んだことにはしない。
                if self.narrowing.has_query() && self.narrowing.matches().is_empty() {
                    self.no_match_confirm = true;
                    return Transition::Continue;
                }
                Transition::DoneOpen {
                    project: self.current_project,
                    index: self.current_index,
                }
            }
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
            Action::Toggle | Action::Ignore => Transition::Continue,
        }
    }

    /// いま動かせるindexの上限。範囲を持たない案件では0から動かさない。
    fn bound(&self) -> u32 {
        self.maximum_index().unwrap_or(0)
    }

    /// 絞り込みが変わった。現在位置を見える案件へ置き直し、一致が無かった警告は役目を終える。
    fn narrowed(&mut self) {
        self.move_to(self.narrowing.anchored(self.current_project));
        self.no_match_confirm = false;
    }

    /// 案件を移る。indexは移った先の範囲へ収める。
    fn move_to(&mut self, project: usize) {
        self.current_project = project;
        self.current_index = self.current_index.min(self.bound());
    }
}

#[cfg(test)]
#[path = "open_selection_test.rs"]
mod open_selection_test;
