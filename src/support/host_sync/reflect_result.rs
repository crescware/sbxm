/// 保存したbranchやtagを、hostのbranchやtagへ反映した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectResult {
    /// hostに無かったbranchやtagができた。
    Created,
    /// hostのbranchが、Sandboxの先端まで早送りで進んだ。
    Updated,
    /// Sandboxのbranchが、hostのbranchより遅れているだけだった。hostは動かさない。
    Behind,
    /// hostとSandboxのbranchが、それぞれ相手に無いcommitを持っていた。
    Diverged,
    /// hostでcheckoutしているbranchであり、hostのgitが動かさなかった。
    CheckedOut,
    /// 同じ名前のtagが、hostで別の先を指していた。
    Exists,
    /// ほかの理由でhostのgitが断った。理由はgitの答えのまま持つ。
    Refused { reason: String },
}
