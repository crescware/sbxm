/// OCI layoutのarchiveが、loadされるimageのdigestを宣言するentry。
///
/// containerd image storeの`docker image save`が書く。legacy layoutには無い。
pub(super) const INDEX_ENTRY: &str = "index.json";
