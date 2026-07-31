use crate::diagnostics::Result;
use crate::paths::{self};

use super::{CONFIG_VERSION, GlobalConfig, RawConfig, RawFile, serialized};

/// configをYAMLへ描画する。
///
/// 引用符の付け方は`yaml_serde`の判断に委ね、sbxmは介入しない。介入するなら危険な値の
/// listを持つか、YAMLを手で組み立てるかのどちらかになる。前者は維持できず、後者はこの
/// 関数がserializeへ移った理由そのものを捨てる。
///
/// 帰結として、出力はYAML 1.2として読まれることを前提とする。`no`や`yes`は引用されず、
/// YAML 1.1の実装から読めばbooleanになる。sbxmは自分が書いたfileを同じcrateで読むため、
/// 往復は一致する。
pub fn render(config: &GlobalConfig) -> Result<String> {
    let raw = RawConfig {
        version: Some(i64::from(CONFIG_VERSION)),
        language: config.language.map(|locale| locale.as_str().to_string()),
        git_user_name: config
            .git_identity
            .as_ref()
            .map(|git| git.user_name.clone()),
        git_user_email: config
            .git_identity
            .as_ref()
            .map(|git| git.user_email.clone()),
        files: config
            .files
            .iter()
            .map(|declaration| RawFile {
                source: Some(paths::display(declaration.source.as_path())),
                destination: Some(paths::display(declaration.destination.as_path())),
            })
            .collect(),
    };
    serialized(&raw, "config.yaml")
}
