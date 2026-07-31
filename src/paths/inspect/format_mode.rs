/// modeを`0o600`のような表示にする。
pub fn format_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}
