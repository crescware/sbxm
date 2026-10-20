/// 件数だけが変わる確認prompt。数以外は完全一致させる。
#[derive(Debug, Clone)]
pub(super) struct CountedPrompt {
    pub(super) prefix: String,
    pub(super) singular_suffix: String,
    pub(super) plural_suffix: String,
}

impl CountedPrompt {
    pub(super) fn matches(&self, buffer: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(buffer) else {
            return false;
        };
        let text = text.strip_suffix(' ').unwrap_or(text);
        let Some(rest) = text.strip_prefix(&self.prefix) else {
            return false;
        };
        let Some((digits, suffix)) = rest.split_once(' ') else {
            return false;
        };
        let Ok(count) = digits.parse::<usize>() else {
            return false;
        };
        count > 0
            && count.to_string() == digits
            && suffix
                == if count == 1 {
                    &self.singular_suffix
                } else {
                    &self.plural_suffix
                }
    }
}
