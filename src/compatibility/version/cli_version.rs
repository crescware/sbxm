/// `<major>.<minor>.<patch>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CliVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl CliVersion {
    /// `0.37.0`のような厳密な3要素表記だけを受け付ける。
    pub fn parse(value: &str) -> Option<CliVersion> {
        let mut parts = value.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(CliVersion {
            major,
            minor,
            patch,
        })
    }

    /// `sbx version`の出力から最初のversion表記を取り出す。
    ///
    /// 数値3要素の並びだけを認め、見つからない場合はparse不能として扱う。
    pub fn extract_from_output(output: &str) -> Option<CliVersion> {
        for token in output.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some(version) = CliVersion::parse(token) {
                return Some(version);
            }
        }
        None
    }
}

impl std::fmt::Display for CliVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
