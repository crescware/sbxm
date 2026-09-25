use std::path::Path;

/// hostにあるrepositoryの案件で、表示に使うrepository名。
///
/// path末尾のdirectory名がcanonical名と同じものを指すなら、その綴りを使う。別の名前で
/// 登録した案件はcanonical名をそのまま使う。索引は表示用の名前を持たないため、この
/// 規則だけで登録時と同じ名前を読み直せる。
pub(super) fn local_name(path: &str, canonical_repository: &str) -> String {
    match Path::new(path).file_name().and_then(|name| name.to_str()) {
        Some(directory) if directory.to_ascii_lowercase() == canonical_repository => {
            directory.to_string()
        }
        _ => canonical_repository.to_string(),
    }
}
