use super::HELPER_PREFIX;

/// この値をsbxm自身が書いたか。
///
/// placeholderの部分は問わない。tokenを登録し直してplaceholderが変わった場合も、
/// 旧版が環境変数を読む形で書いていた場合も、書き換えてよい対象として扱う。
/// 形が違う値は別の利用者の設定かもしれないため、書き換えない。
pub(super) fn is_sbxm_helper(value: &str) -> bool {
    value.starts_with(HELPER_PREFIX)
}
