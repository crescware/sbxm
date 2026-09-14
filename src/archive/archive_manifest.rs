/// archiveが宣言するimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveManifest {
    /// archiveへ保存されたときのtag。
    pub repo_tags: Vec<String>,
    /// image configのdigest。archive内でconfigを指す名前でもある。
    ///
    /// `docker image inspect`の`Id`とも、runtimeがloadしたTemplateへ報告するidとも
    /// 別物である。buildがOCI image indexを作る構成では、どちらもindexのdigestになり、
    /// この値と一致しない。同一性の照合には[`super::read_image_ids`]を使う。
    pub config_digest: String,
    /// archive内でimage configを指すentry名。
    pub config_entry: String,
}
