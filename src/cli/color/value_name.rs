use std::sync::OnceLock;

use super::modes;

/// `--color`のvalue name。CLI parser libraryが`&'static str`を要求するため一度だけ組む。
pub(super) fn value_name() -> &'static str {
    static VALUE_NAME: OnceLock<String> = OnceLock::new();
    VALUE_NAME.get_or_init(|| modes().join("|")).as_str()
}
