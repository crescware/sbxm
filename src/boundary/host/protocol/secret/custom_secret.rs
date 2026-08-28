/// `sbx secret ls`が示すcustom secretの登録。
///
/// scope、対象host、環境変数名、placeholderを持つ。`SECRET`列は読まない。tokenの一部が
/// 現れるためである。
#[derive(Debug, PartialEq, Eq)]
pub struct CustomSecret {
    /// この登録が属するscope。
    ///
    /// Sandboxへ結び付いた登録はそのSandbox名を示す。global scopeの登録はどのSandboxでも
    /// 使われるため、1案件の後片付けで消してよい対象と区別する必要がある。
    pub scope: String,
    /// proxyが認証を差し替える対象host。
    pub targets: Vec<String>,
    /// placeholderを受け取るSandbox内の環境変数名。
    pub env: String,
    /// Sandboxが実際に見る値。
    ///
    /// tokenそのものではなく、tokenの居場所を指す公開の目印である。同じenvへ登録を
    /// やり直すとき、この値を`--placeholder`へ渡せばSandboxが持つ値と一致したまま
    /// 更新できる。読むのはそのためだけであり、隣の`SECRET`列は読まない。
    pub placeholder: String,
}
