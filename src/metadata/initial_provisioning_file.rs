/// 初回構築中に固定した、宣言file 1件の検証可能なsnapshot。
///
/// fileの内容は保存しない。sourceのpath、Sandbox内のdestination、内容のdigestを
/// 保存し、復旧時に現在のglobal configが同じ入力を指していることだけを確かめる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProvisioningFile {
    pub source: String,
    pub destination: String,
    pub sha256: String,
}
