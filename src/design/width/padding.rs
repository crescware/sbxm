use super::display_width;

/// 指定幅へ届くまでの余白。装飾を載せた文字列へ後から連結する。
pub fn padding(text: &str, width: usize) -> String {
    " ".repeat(width.saturating_sub(display_width(text)))
}
