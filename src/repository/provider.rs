/// repositoryをhostしているservice。
///
/// 初期versionはGitHubだけを持つ。値はconfigやmetadataへ保存するため翻訳しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Github,
}

impl Provider {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Github => "github",
        }
    }

    /// 保存済みの値を読む。未知のproviderは推測せず`None`とする。
    pub fn parse(value: &str) -> Option<Provider> {
        match value {
            "github" => Some(Provider::Github),
            _ => None,
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
