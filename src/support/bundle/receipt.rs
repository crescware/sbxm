use super::ReceivedBundle;

/// Sandboxへbundleを求めた結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receipt {
    /// Sandboxが保存するrefを1つも持たない。
    Nothing,
    /// Sandboxのrefが、hostに保存済みのものと同じだった。bundleは運んでいない。
    Unchanged,
    /// 受け取ったbundle。
    Bundle(ReceivedBundle),
}
