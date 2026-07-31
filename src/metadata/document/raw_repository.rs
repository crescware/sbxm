use serde::{Deserialize, Serialize};

/// 登録対象の不変なrepository identity。
///
/// clone URLからtransportを実行時に推測し直さないよう、解釈済みの値をそのまま持つ。
#[derive(Debug, Deserialize, Serialize)]
pub struct RawRepository {
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub canonical_id: Option<String>,
    pub clone_transport: Option<String>,
    pub clone_url: Option<String>,
}
