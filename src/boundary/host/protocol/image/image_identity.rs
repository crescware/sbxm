/// `docker image inspect`から読むimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageIdentity {
    /// `sha256:<hex>`。archiveのconfig blobと同じ値になる。
    pub id: String,
    pub labels: std::collections::BTreeMap<String, String>,
}
