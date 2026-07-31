use crate::diagnostics::Msg;

use super::Cell;

/// 列名と行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    headers: Vec<Msg>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    /// 列名を決めて空のtableを作る。
    pub fn new(headers: Vec<Msg>) -> Table {
        Table {
            headers,
            rows: Vec::new(),
        }
    }

    /// 1行を足す。列数がheaderと違っても描画は止めず、足りない列は空欄として扱う。
    pub fn push(&mut self, cells: Vec<Cell>) {
        self.rows.push(cells);
    }

    /// 1行を足した自身を返す。
    #[cfg(test)]
    pub fn row(mut self, cells: Vec<Cell>) -> Table {
        self.push(cells);
        self
    }

    /// 行が1件もないか。空のtableはsectionごと省く判断に使う。
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn headers(&self) -> &[Msg] {
        &self.headers
    }

    pub fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    /// 列数。headerと行のうち最も広いものに合わせる。
    pub(super) fn columns(&self) -> usize {
        self.rows
            .iter()
            .map(std::vec::Vec::len)
            .chain(std::iter::once(self.headers.len()))
            .max()
            .unwrap_or(0)
    }
}
