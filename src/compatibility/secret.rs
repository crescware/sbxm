//! `sbx secret ls`の解釈。

use crate::error::Result;

use super::json::unparseable;

/// `sbx secret ls`が示すcustom secretの登録。
///
/// scope、対象host、環境変数名、placeholderを持つ。`SECRET`列は読まない。tokenの一部が
/// 現れるためである。
#[derive(Debug, PartialEq, Eq)]
pub struct CustomSecret {
    /// この登録が属するscope。
    ///
    /// Sandboxへ結び付いた登録はそのSandbox名を示す。global scopeの登録はどのSandboxでも
    /// 使われるため、1案件の後片付けで消してよい対象と区別する必要がある。
    pub scope: String,
    /// proxyが認証を差し替える対象host。
    pub targets: Vec<String>,
    /// placeholderを受け取るSandbox内の環境変数名。
    pub env: String,
    /// Sandboxが実際に見る値。
    ///
    /// tokenそのものではなく、tokenの居場所を指す公開の目印である。同じenvへ登録を
    /// やり直すとき、この値を`--placeholder`へ渡せばSandboxが持つ値と一致したまま
    /// 更新できる。読むのはそのためだけであり、隣の`SECRET`列は読まない。
    pub placeholder: String,
}

/// custom secretの表が始まる見出し。
const CUSTOM_SECRETS_HEADING: &str = "CUSTOM SECRETS";

/// `sbx secret ls`が示すcustom secretを読む。
///
/// 値は取得も表示もしない。存在と宛先だけを読む。service secretとregistry secretは
/// 別の表に並び、custom secretとは扱いが異なるため読み飛ばす。
pub fn parse_custom_secrets(output: &str) -> Result<Vec<CustomSecret>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(unparseable("sbx secret ls", "the output is empty"));
    }
    // 1件もない場合は表ではなく文で示す。
    if trimmed.starts_with("No secrets found") {
        return Ok(Vec::new());
    }

    let mut lines = trimmed.lines();
    // 見出しがない出力は、custom secretが1件もないことを示す。`any`は一致した行まで
    // 読み進めるので、続く`find`が見出し直後の行から始まる。
    if !lines.any(|line| line.trim() == CUSTOM_SECRETS_HEADING) {
        return Ok(Vec::new());
    }
    let header = lines
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| unparseable("sbx secret ls", "the custom secret listing has no header"))?;
    let columns = table_fields(header);
    let column_of = |wanted: &str| -> Result<usize> {
        columns
            .iter()
            .position(|column| *column == wanted)
            .ok_or_else(|| {
                unparseable(
                    "sbx secret ls",
                    &format!("the custom secret listing has no {wanted} column"),
                )
            })
    };
    let scope_at = column_of("SCOPE")?;
    let targets_at = column_of("TARGETS")?;
    let env_at = column_of("ENV")?;
    let placeholder_at = column_of("PLACEHOLDER")?;

    let mut customs = Vec::new();
    for line in lines {
        // 空行が表の終わりを示す。
        if line.trim().is_empty() {
            break;
        }
        let fields = table_fields(line);
        // 列数の合わない行からは、どの値がどの列かを決められない。
        if fields.len() != columns.len() {
            return Err(unparseable(
                "sbx secret ls",
                &format!(
                    "a custom secret row holds {} values for {} columns",
                    fields.len(),
                    columns.len()
                ),
            ));
        }
        customs.push(CustomSecret {
            scope: fields[scope_at].to_string(),
            targets: fields[targets_at]
                .split([',', ' '])
                .filter(|target| !target.is_empty())
                .map(str::to_string)
                .collect(),
            env: fields[env_at].to_string(),
            placeholder: fields[placeholder_at].to_string(),
        });
    }
    Ok(customs)
}

/// 桁を揃えた表の1行を列へ分ける。
///
/// 区切りは2つ以上の空白とする。1つの列が複数の値を空白で並べることがあり、
/// 空白1つで切ると列の対応が崩れる。
fn table_fields(line: &str) -> Vec<&str> {
    line.split("  ")
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;
