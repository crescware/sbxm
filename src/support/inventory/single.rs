use crate::compatibility::SandboxEntry;
use crate::diagnostics::Result;

use super::duplicated;

/// 名前が一致するentryを1件だけ取り出す。
///
/// 同名が複数ある一覧からは、どれがこの案件のSandboxかを決められない。先頭を選んで
/// 続けると、別のSandboxのsessionやstateを読んだまま削除へ進み得る。
pub fn single<'a>(entries: &'a [SandboxEntry], name: &str) -> Result<Option<&'a SandboxEntry>> {
    let matched: Vec<&SandboxEntry> = entries.iter().filter(|entry| entry.name == name).collect();
    match matched.as_slice() {
        [] => Ok(None),
        [entry] => Ok(Some(entry)),
        _ => Err(duplicated(&[name])),
    }
}
