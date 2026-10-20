/// `sbx ls --json`の出力が、JSONとして読み終わる前に途切れているか。
///
/// 途中で切れた出力だけを、振り直せば変わりうる失敗とする。JSONとして読み終えた
/// うえで形が違う出力は、何度読み直しても同じ形のまま返る。
pub fn is_listing_cut_short(output: &str) -> bool {
    serde_json::from_str::<serde::de::IgnoredAny>(output.trim()).is_err_and(|error| error.is_eof())
}
