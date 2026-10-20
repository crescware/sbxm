use crate::diagnostics::{Result, unparseable};

use crate::boundary::host::protocol::json::string_field;

use super::{SandboxEntry, SandboxState};

/// `sbx ls --json`のstructured outputをparseする。
///
/// sbx v0.48.0以降が返す配列を読む。未知のstateやworkspaceの形は推測しない。
pub fn parse_sandbox_list(output: &str) -> Result<Vec<SandboxEntry>> {
    let documents = sandbox_documents(output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx ls", "an entry is not an object"))?;

        let name = string_field(object, "name")
            .filter(|name| !name.is_empty())
            .ok_or_else(|| unparseable("sbx ls", "an entry has no name"))?;

        let observed = string_field(object, "status")
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

/// sbx v0.48.0はSandboxの配列を直接返す。
fn sandbox_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    let document: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|error| unparseable("sbx ls", &error.to_string()))?;
    match document {
        serde_json::Value::Array(items) => Ok(items),
        _ => Err(unparseable("sbx ls", "the document is not an array")),
    }
}

/// `Sandboxが使っているWorkspace`。
///
/// sbx v0.48.0は`workspaces`を配列で示す。sbxmが作るSandboxは中立Workspaceを
/// 1つだけ持つため、2つ以上ある一覧からはこの案件の成果物と判定しない。
fn workspace_of(object: &serde_json::Map<String, serde_json::Value>) -> Result<Option<String>> {
    match object.get("workspaces") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(items)) => match items.as_slice() {
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
        Some(_) => Err(unparseable("sbx ls", "workspaces is not a list")),
    }
}
