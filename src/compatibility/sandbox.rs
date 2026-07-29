//! `sbx ls --json`の解釈。

use crate::error::Result;

use super::json::{string_field, unparseable, wrapped_documents};

/// Sandboxのruntime状態。未知の値を既知の値へ丸めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    Running,
    Stopped,
}

impl SandboxState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            SandboxState::Running => "running",
            SandboxState::Stopped => "stopped",
        }
    }
}

/// `sbx ls`が示すSandbox 1件。
///
/// 対応versionの`sbx ls --json`は、元にしたTemplateも接続中のsession数も示さない。
/// 示されない値をOptionで持つと、その`None`をどう読むかを利用側がそれぞれ決めることに
/// なる。読めないものは持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEntry {
    pub name: String,
    pub state: SandboxState,
    /// runtimeが示したままのstate。管理外Sandboxの表示に使う。
    pub raw_state: String,
    /// Sandboxへ渡したworkspace。示されない場合は`None`。
    pub workspace: Option<String>,
}

/// `sbx ls --json`のstructured outputをparseする。
pub fn parse_sandbox_list(output: &str) -> Result<Vec<SandboxEntry>> {
    let documents = sandbox_documents(output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx ls", "an entry is not an object"))?;

        let name = string_field(object, "name")
            .or_else(|| string_field(object, "Name"))
            .filter(|name| !name.is_empty())
            .ok_or_else(|| unparseable("sbx ls", "an entry has no name"))?;

        let observed = string_field(object, "state")
            .or_else(|| string_field(object, "status"))
            .or_else(|| string_field(object, "State"))
            .or_else(|| string_field(object, "Status"))
            .ok_or_else(|| unparseable("sbx ls", &format!("sandbox {name} has no state")))?;
        let state = match observed.to_ascii_lowercase().as_str() {
            "running" => SandboxState::Running,
            "stopped" => SandboxState::Stopped,
            other => {
                return Err(unparseable(
                    "sbx ls",
                    &format!("state {other} has no defined meaning in this build"),
                ));
            }
        };

        let workspace = workspace_of(object)?;

        entries.push(SandboxEntry {
            name,
            state,
            raw_state: observed,
            workspace,
        });
    }
    Ok(entries)
}

/// `sbx ls --json`が並べるSandbox。
///
/// 対象versionは`{"sandboxes": [...]}`で包む。
fn sandbox_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    wrapped_documents("sbx ls", "sandboxes", output)
}

/// Sandboxが使っているWorkspace。
///
/// 対象versionは`workspaces`を配列で示す。sbxmが作るSandboxは中立Workspaceを
/// 1つだけ持つため、2つ以上ある一覧からはこの案件の成果物と判定しない。
fn workspace_of(object: &serde_json::Map<String, serde_json::Value>) -> Result<Option<String>> {
    let listed = object
        .get("workspaces")
        .or_else(|| object.get("Workspaces"));
    let Some(listed) = listed else {
        return Ok(string_field(object, "workspace")
            .or_else(|| string_field(object, "Workspace"))
            .filter(|value| !value.is_empty()));
    };

    match listed {
        serde_json::Value::Array(items) => match items.as_slice() {
            [] => Ok(None),
            [only] => {
                let value = only
                    .as_str()
                    .ok_or_else(|| unparseable("sbx ls", "a workspace is not a string"))?;
                Ok(Some(value.to_string()).filter(|value| !value.is_empty()))
            }
            _ => Err(unparseable(
                "sbx ls",
                &format!(
                    "the sandbox works in {} workspaces instead of one",
                    items.len()
                ),
            )),
        },
        serde_json::Value::Null => Ok(None),
        _ => Err(unparseable("sbx ls", "workspaces is not a list")),
    }
}

#[cfg(test)]
#[path = "sandbox_test.rs"]
mod sandbox_test;
