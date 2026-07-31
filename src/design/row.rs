use crate::design::width::padding;

/// 1行分のcellを、列幅にそろえて並べる。末尾の余白は残さない。
///
/// cellは`(装飾前, 装飾後)`で受け取る。余白は装飾前の幅から決めるため、色のon/offで
/// 列の開始位置が変わらない。
pub(super) fn row(cells: &[(String, String)], widths: &[usize]) -> String {
    let mut out = String::new();
    for (index, (plain, painted)) in cells.iter().enumerate() {
        out.push_str(painted);
        if index + 1 < cells.len() {
            out.push_str(&padding(plain, widths.get(index).copied().unwrap_or(0)));
        }
    }
    out
}
