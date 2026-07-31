use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::Entry;

/// `NUL区切りのporcelain出力をparseする`。
///
/// 空のfieldがrecordの区切りとなる。pathを持たないrecordは受け付けない。
pub fn parse_list(output: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut current: Option<Entry> = None;

    for field in output.split('\0') {
        if field.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        let (key, value) = match field.split_once(' ') {
            Some((key, value)) => (key, value),
            None => (field, ""),
        };
        match key {
            "worktree" => {
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                current = Some(Entry {
                    path: value.to_string(),
                    bare: false,
                    detached: false,
                });
            }
            "bare" | "detached" => match current.as_mut() {
                Some(entry) => {
                    if key == "bare" {
                        entry.bare = true;
                    } else {
                        entry.detached = true;
                    }
                }
                None => {
                    return Err(Error::new(
                        ErrorId::ExternalOutputUnparseable,
                        msg!(
                            "error-external-output-unparseable",
                            program = "git worktree list",
                            detail = format!("{key} appears before any worktree")
                        ),
                    ));
                }
            },
            _ => {}
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    Ok(entries)
}
