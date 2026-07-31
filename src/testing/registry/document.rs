use super::Entry;

/// entryを並べたregistry document。
pub fn document(entries: &[Entry]) -> String {
    let body: String = entries.iter().map(Entry::text).collect();
    format!("version: 1\nprojects:\n{body}")
}
