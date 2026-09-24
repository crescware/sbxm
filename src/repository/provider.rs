/// repositoryをhostしているservice。
///
/// 値はconfigやmetadataへ保存するため翻訳しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Github,
    /// 利用者のhostにあるrepository。hostの`.git`がoriginの役を持つ。
    Local,
}

impl Provider {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Github => "github",
            Provider::Local => "local",
        }
    }

    /// 保存済みの値を読む。未知のproviderは推測せず`None`とする。
    pub fn parse(value: &str) -> Option<Provider> {
        match value {
            "github" => Some(Provider::Github),
            "local" => Some(Provider::Local),
            _ => None,
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
