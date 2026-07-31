use std::fmt::Write as _;

/// 1 entryのfieldを、書き出す順のまま持つ。
pub struct Entry {
    fields: Vec<(String, String)>,
}

impl Entry {
    /// 仕様が例示する1 entry。
    pub fn example() -> Entry {
        Entry::new(
            "example-org/alpha",
            "/home/user/Projects/alpha.project",
            "ssh",
            "git@github.com:Example-Org/Alpha.git",
        )
    }

    /// canonical `IDとproject` rootとtransportから組み立てる。
    pub fn of(canonical_id: &str, project_root: &str, transport: &str) -> Entry {
        let clone_url = match transport {
            "ssh" => format!("git@github.com:{canonical_id}.git"),
            _ => format!("https://github.com/{canonical_id}.git"),
        };
        Entry::new(canonical_id, project_root, transport, &clone_url)
    }

    fn new(canonical_id: &str, project_root: &str, transport: &str, clone_url: &str) -> Entry {
        Entry {
            fields: [
                ("canonical_id", canonical_id),
                ("project_root", project_root),
                ("provider", "github"),
                ("clone_transport", transport),
                ("clone_url", clone_url),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        }
    }

    /// 1 fieldを落とす。欠落したfieldの診断を確かめるために使う。
    pub fn without(mut self, field: &str) -> Entry {
        self.fields.retain(|(key, _)| key != field);
        self
    }

    /// 1 fieldの値を差し替える。
    pub fn with(mut self, field: &str, value: &str) -> Entry {
        for (key, stored) in &mut self.fields {
            if key == field {
                *stored = value.to_string();
            }
        }
        self
    }

    /// sequence itemとしてのYAML。
    pub fn text(&self) -> String {
        self.fields
            .iter()
            .enumerate()
            .fold(String::new(), |mut out, (index, (key, value))| {
                let prefix = if index == 0 { "- " } else { "  " };
                // Stringへの書き込みは失敗しない。
                let _ = writeln!(out, "{prefix}{key}: {value}");
                out
            })
    }
}
