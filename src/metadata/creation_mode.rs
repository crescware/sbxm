/// 全managed worktreeの作り方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationMode {
    /// remote default branchをtrackingするlocal branchを1つ作る。
    Attached,
    /// 全managed worktreeを同じ`origin/<BRANCH>` commitから作る。
    Detached,
}

impl CreationMode {
    /// 翻訳しない安定した表記。metadataと利用者向けtableの両方で使う。
    pub fn as_str(self) -> &'static str {
        match self {
            CreationMode::Attached => "attached",
            CreationMode::Detached => "detached",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<CreationMode> {
        match value {
            "attached" => Some(CreationMode::Attached),
            "detached" => Some(CreationMode::Detached),
            _ => None,
        }
    }
}

impl std::fmt::Display for CreationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
