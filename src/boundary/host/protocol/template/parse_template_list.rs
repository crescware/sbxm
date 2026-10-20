use crate::diagnostics::{Result, unparseable};

use crate::boundary::host::protocol::json::string_field;

use super::TemplateEntry;

/// `sbx template ls --json`のstructured outputをparseする。
///
/// sbx v0.48.0以降が返す配列だけを読む。名前を持たないentryがある出力は、
/// 一覧として信用できないためparse不能として扱う。
pub fn parse_template_list(output: &str) -> Result<Vec<TemplateEntry>> {
    let documents = template_documents(output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx template ls", "an entry is not an object"))?;

        // runtimeのimage storeはrepositoryとtagで1件を示す。
        let repository = string_field(object, "repository")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unparseable("sbx template ls", "an entry has no repository"))?;
        let tag = string_field(object, "tag")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                unparseable(
                    "sbx template ls",
                    &format!("the entry for {repository} has no tag"),
                )
            })?;

        let id = string_field(object, "id").filter(|value| !value.is_empty());

        entries.push(TemplateEntry {
            names: reference_names(&repository, &tag),
            id,
        });
    }
    Ok(entries)
}

/// `sbx template ls --json`が並べるimage。
///
/// sbx v0.48.0はimageの配列を直接返し、1件もなければ空の配列を返す。旧版の包みや
/// 1行1件のJSON、空の出力は受け付けない。
fn template_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    let document: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|error| unparseable("sbx template ls", &error.to_string()))?;
    match document {
        serde_json::Value::Array(items) => Ok(items),
        _ => Err(unparseable(
            "sbx template ls",
            "the document is not an array",
        )),
    }
}

/// 同じimageを指す参照の書き方。
///
/// runtimeは`docker.io/library/`を補って表示する。sbxmが渡す名前は補われる前の
/// 表記であるため、両方を同じimageの名前として扱う。
fn reference_names(repository: &str, tag: &str) -> Vec<String> {
    let mut names = vec![format!("{repository}:{tag}")];
    for prefix in ["docker.io/library/", "docker.io/"] {
        if let Some(short) = repository.strip_prefix(prefix) {
            names.push(format!("{short}:{tag}"));
            break;
        }
    }
    names
}
