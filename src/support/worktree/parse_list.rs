use crate::diagnostics::{Result, unparseable};

use super::Entry;

/// `git worktree list --porcelain -z`を、record単位で厳密にparseする。
///
/// 成功終了したGitの出力でも、未知のfield、record境界の欠落、空のrecordは受け付け
/// ない。worktree一覧を読めないことを、worktreeが無いこととして破壊処理へ渡さない。
pub fn parse_list(output: &str) -> Result<Vec<Entry>> {
    if output.is_empty() || !output.ends_with("\0\0") {
        return Err(unparseable(
            "git worktree list",
            "the porcelain output did not end at a complete record",
        ));
    }

    // `-z`はfieldごとにNUL、recordごとに空field（NUL NUL）を出す。最後のrecord
    // separatorを除いてから分割するため、途中の空recordも検出できる。
    let body = &output[..output.len() - 2];
    if body.is_empty() {
        return Err(unparseable(
            "git worktree list",
            "the porcelain output contained no record",
        ));
    }

    let mut entries = Vec::new();
    for record in body.split("\0\0") {
        if record.is_empty() {
            return Err(unparseable(
                "git worktree list",
                "the porcelain output contained an empty record",
            ));
        }
        entries.push(parse_record(record)?);
    }
    Ok(entries)
}

fn parse_record(record: &str) -> Result<Entry> {
    let mut fields = record.split('\0');
    let first = fields.next().unwrap_or_default();
    let Some(path) = first.strip_prefix("worktree ") else {
        return Err(unparseable(
            "git worktree list",
            "a record did not begin with worktree",
        ));
    };
    if path.is_empty() {
        return Err(unparseable(
            "git worktree list",
            "a worktree record had no path",
        ));
    }

    let mut entry = Entry {
        path: path.to_string(),
        bare: false,
        detached: false,
    };
    let flags = parse_fields(fields, &mut entry)?;
    if !flags.saw_bare && !flags.saw_branch && !flags.saw_detached {
        return Err(unparseable(
            "git worktree list",
            "a record had no worktree state field",
        ));
    }
    if flags.saw_bare && (flags.saw_head || flags.saw_branch || flags.saw_detached) {
        return Err(unparseable(
            "git worktree list",
            "a bare record had an invalid set of fields",
        ));
    }
    Ok(entry)
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct RecordFlags {
    saw_head: bool,
    saw_branch: bool,
    saw_detached: bool,
    saw_bare: bool,
    saw_locked: bool,
    saw_prunable: bool,
}

fn parse_fields<'a>(
    fields: impl Iterator<Item = &'a str>,
    entry: &mut Entry,
) -> Result<RecordFlags> {
    let mut flags = RecordFlags::default();
    for field in fields {
        let (key, value) = field
            .split_once(' ')
            .map_or((field, None), |(key, value)| (key, Some(value)));
        match key {
            "HEAD" => {
                if flags.saw_head || !valid_single_value(value) {
                    return Err(unparseable(
                        "git worktree list",
                        "a record had a missing or duplicate HEAD field",
                    ));
                }
                flags.saw_head = true;
            }
            "branch" => {
                if flags.saw_branch || flags.saw_detached || !valid_single_value(value) {
                    return Err(unparseable(
                        "git worktree list",
                        "a record had a missing or duplicate branch field",
                    ));
                }
                flags.saw_branch = true;
            }
            "detached" => {
                if flags.saw_detached || flags.saw_branch || value.is_some() {
                    return Err(unparseable(
                        "git worktree list",
                        "a record had an invalid detached field",
                    ));
                }
                flags.saw_detached = true;
                entry.detached = true;
            }
            "bare" => {
                if flags.saw_bare || value.is_some() || flags.saw_branch || flags.saw_detached {
                    return Err(unparseable(
                        "git worktree list",
                        "a record had an invalid bare field",
                    ));
                }
                flags.saw_bare = true;
                entry.bare = true;
            }
            // These fields are valid even though Entry does not need to retain their reason.
            "locked" | "prunable" => {
                let seen = if key == "locked" {
                    &mut flags.saw_locked
                } else {
                    &mut flags.saw_prunable
                };
                if *seen {
                    return Err(unparseable(
                        "git worktree list",
                        "a record had a duplicate lock or prune field",
                    ));
                }
                *seen = true;
            }
            _ => {
                return Err(unparseable(
                    "git worktree list",
                    "a record contained an unknown field",
                ));
            }
        }
    }
    Ok(flags)
}

/// `HEAD`とbranch refは単一の空白なしvalueでなければならない。pathやlock/pruneの
/// 理由は空白を含みうるため、record全体へ一律にこの制約を適用しない。
fn valid_single_value(value: Option<&str>) -> bool {
    matches!(value, Some(value) if !value.is_empty() && !value.chars().any(char::is_whitespace))
}
