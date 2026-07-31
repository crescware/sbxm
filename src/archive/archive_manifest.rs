/// archiveが宣言するimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveManifest {
    /// archiveへ保存されたときのtag。
    pub repo_tags: Vec<String>,
    /// image configのdigest。archive内でconfigを指す名前でもある。
    ///
    /// `docker image inspect`の`Id`とは別物である。buildがOCI image indexを
    /// 作る構成では、`Id`はindexのdigestになり、この値と一致しない。
    pub config_digest: String,
    /// archive内でimage configを指すentry名。
    pub config_entry: String,
}
