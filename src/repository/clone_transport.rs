/// host cloneが使うclone方式。
///
/// SSHとHTTPSは同じrepositoryを指していても別の構成として扱う。暗黙に変換しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneTransport {
    Ssh,
    Https,
}

impl CloneTransport {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            CloneTransport::Ssh => "ssh",
            CloneTransport::Https => "https",
        }
    }

    /// 保存済みの値を読む。未知のtransportは推測せず`None`とする。
    pub fn parse(value: &str) -> Option<CloneTransport> {
        match value {
            "ssh" => Some(CloneTransport::Ssh),
            "https" => Some(CloneTransport::Https),
            _ => None,
        }
    }
}

impl std::fmt::Display for CloneTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
