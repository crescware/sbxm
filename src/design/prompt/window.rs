/// 現在位置が見える範囲の候補index。
///
/// 端末の高さを超える候補があっても、markerと状態を残したまま窓だけを動かす。
pub fn window(count: usize, current: usize, viewport: Option<usize>) -> std::ops::Range<usize> {
    let Some(visible) = viewport.filter(|visible| *visible < count) else {
        return 0..count;
    };
    let start = current.saturating_sub(visible / 2).min(count - visible);
    start..start + visible
}
