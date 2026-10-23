/// `INSIDE_CHECK`が並べた、1つのcommandの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsideRecord {
    /// commandの終了status。shellの`$?`であり、0から255の値である。
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
