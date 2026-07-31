use std::sync::OnceLock;

use super::tags;

/// `--lang`のvalue name。CLI parser libraryが`&'static str`を要求するため一度だけ組む。
pub(super) fn value_name() -> &'static str {
    static VALUE_NAME: OnceLock<String> = OnceLock::new();
    VALUE_NAME.get_or_init(|| tags().join("|")).as_str()
}
