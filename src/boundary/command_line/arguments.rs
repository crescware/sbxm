use std::collections::BTreeMap;

/// parser libraryから切り離した一回分のargument values。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arguments {
    values: BTreeMap<&'static str, Vec<String>>,
    flags: BTreeMap<&'static str, bool>,
}

impl Arguments {
    pub(crate) fn insert_value(&mut self, id: &'static str, value: String) {
        self.values.entry(id).or_default().push(value);
    }

    pub(crate) fn insert_flag(&mut self, id: &'static str, value: bool) {
        self.flags.insert(id, value);
    }

    pub fn value(&self, id: &'static str) -> Option<&str> {
        self.values
            .get(id)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    pub fn values(&self, id: &'static str) -> impl Iterator<Item = &str> {
        self.values
            .get(id)
            .into_iter()
            .flat_map(|values| values.iter().map(String::as_str))
    }

    pub fn flag(&self, id: &'static str) -> bool {
        self.flags.get(id).copied().unwrap_or(false)
    }
}
