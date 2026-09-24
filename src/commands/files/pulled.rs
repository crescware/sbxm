use crate::config::FileDeclaration;
use crate::support::files::ReceivedCopy;
use crate::support::select::Locked;

/// Sandboxから取り出した宣言fileと、採用するかを決めるのに要るもの。
///
/// 決めるまでproject lockを持ち続ける。その間に別の実行がbaselineを書き換えない。
pub struct Pulled {
    pub declaration: FileDeclaration,
    /// 取り出した案件の`<owner>/<repository>`。
    pub project: String,
    pub copy: ReceivedCopy,
    /// 取り出した時点の、hostの宣言fileのdigest。
    pub host_sha256: String,
    /// 取り出した案件のほかに登録されている案件の数。
    pub others: usize,
    pub locked: Locked,
}
