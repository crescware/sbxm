use super::*;

impl Table {
    /// 1行を足した自身を返す。
    pub fn row(mut self, cells: Vec<Cell>) -> Table {
        self.push(cells);
        self
    }
}
