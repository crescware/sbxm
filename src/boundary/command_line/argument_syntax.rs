use super::ArgumentAction;

/// parser libraryへ変換する前のargument定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentSyntax {
    pub(crate) id: &'static str,
    pub(crate) long: Option<&'static str>,
    pub(crate) short: Option<char>,
    pub(crate) value_name: Option<&'static str>,
    pub(crate) action: ArgumentAction,
    pub(crate) required: bool,
    pub(crate) help: String,
}

impl ArgumentSyntax {
    pub fn value(id: &'static str, help: String) -> Self {
        Self {
            id,
            long: None,
            short: None,
            value_name: None,
            action: ArgumentAction::Value,
            required: false,
            help,
        }
    }

    pub fn flag(id: &'static str, help: String) -> Self {
        Self {
            id,
            long: None,
            short: None,
            value_name: None,
            action: ArgumentAction::Flag,
            required: false,
            help,
        }
    }

    pub fn append(id: &'static str, help: String) -> Self {
        Self {
            action: ArgumentAction::Append,
            ..Self::value(id, help)
        }
    }

    pub fn long(mut self, name: &'static str) -> Self {
        self.long = Some(name);
        self
    }

    pub fn short(mut self, name: char) -> Self {
        self.short = Some(name);
        self
    }

    pub fn value_name(mut self, name: &'static str) -> Self {
        self.value_name = Some(name);
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}
