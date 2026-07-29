//! `sbx template ls --json`の解釈。

use crate::error::Result;

use super::json::{string_field, unparseable, wrapped_documents};

/// `sbx template ls`が示すTemplate 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    /// このentryを指す名前。registry prefixを補う前後の両方を持つ。
    pub names: Vec<String>,
}

impl TemplateEntry {
    /// 与えられた参照がこのentryを指すか。
    pub fn is_named(&self, reference: &str) -> bool {
        self.names.iter().any(|name| name == reference)
    }
}

/// `sbx template ls`のstructured outputをparseする。
///
/// 一覧形式と、1行1件のJSON形式のどちらでも読む。名前を持たないentryがある出力は、
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
            .or_else(|| string_field(object, "Repository"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unparseable("sbx template ls", "an entry has no repository"))?;
        let tag = string_field(object, "tag")
            .or_else(|| string_field(object, "Tag"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                unparseable(
                    "sbx template ls",
                    &format!("the entry for {repository} has no tag"),
                )
            })?;

        entries.push(TemplateEntry {
            names: reference_names(&repository, &tag),
        });
    }
    Ok(entries)
}

/// `sbx template ls --json`が並べるimage。
///
/// 対象versionは`{"images": [...]}`で包む。
fn template_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    wrapped_documents("sbx template ls", "images", output)
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

#[cfg(test)]
#[path = "template_test.rs"]
mod template_test;
