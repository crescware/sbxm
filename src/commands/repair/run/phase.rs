/// 端末へ表示するrepairの段階。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Plan,
    Fresh,
    Healthy,
    Repaired,
}
