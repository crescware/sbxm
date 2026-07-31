/// 比較に使うASCII lowercaseの`<owner>/<repository>`。
///
/// 案件の同一性はこの形式だけで判定する。表示には[`ProjectId`]の表記を使う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalProjectId {
    pub(super) value: String,
    /// `value`の中のslashのbyte位置。[`ProjectId`]から持ち越す。
    pub(super) slash: usize,
}

impl CanonicalProjectId {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// lowercase化したrepository。host pathとSandbox内pathに使う。
    pub fn repository(&self) -> &str {
        self.value.get(self.slash + 1..).unwrap_or_default()
    }
}

impl std::fmt::Display for CanonicalProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}
