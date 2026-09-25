use super::clone_directory_name;

/// hostにあるrepositoryの案件で、表示に使うrepository名。
///
/// `git clone`が作るdirectoryの名前がcanonical名と同じものを指すなら、その綴りを使う。
/// 別の名前で登録した案件はcanonical名をそのまま使う。索引は表示用の名前を持たない
/// ため、この規則だけで登録時と同じ名前を読み直せる。
pub(super) fn local_name(path: &str, canonical_repository: &str) -> String {
    match clone_directory_name(path) {
        Some(directory) if directory.to_ascii_lowercase() == canonical_repository => {
            directory.to_string()
        }
        _ => canonical_repository.to_string(),
    }
}
