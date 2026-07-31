use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

/// `Raw型をYAMLへ落とす`。
///
/// `Raw型は文字列と整数とVecだけで構成されるため、serializeは成立する`。成立しない値が
/// 混ざったときに黙って壊れた記録を書かないよう、失敗は内部errorとして表に出す。
pub fn serialized<T: serde::Serialize>(raw: &T, document: &str) -> Result<String> {
    yaml_serde::to_string(raw).map_err(|error| {
        Error::new(
            ErrorId::DocumentRenderFailed,
            msg!(
                "error-document-render-failed",
                document = document,
                detail = error
            ),
        )
    })
}
