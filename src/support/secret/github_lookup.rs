use crate::diagnostics::Error;

use super::GithubRegistration;

/// このSandboxが使える`GitHub` tokenの登録を探した結果。
///
/// 登録が1件も無いことと、複数あって1件に決まらないことを分けて持つ。前者は利用者が
/// 登録すれば埋まる欠けであり、後者はどのplaceholderを使うかを利用者が決める必要がある。
/// どちらも、登録を使う工程は同じ診断で止める。
#[derive(Debug, Clone)]
pub enum GithubLookup {
    /// 1件に決まった。
    Registered(GithubRegistration),
    /// 1件も無い。登録するcommandを示す診断を持つ。
    Unregistered(Error),
    /// 複数あり、どれを使うか決まらない。余分な登録を消すcommandを示す診断を持つ。
    Ambiguous(Error),
}
