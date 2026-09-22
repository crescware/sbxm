/// Dockerへのloginが必要だと明示された失敗だけを認識する。
///
/// 単なる401、secret不在、daemonへの接続失敗から未loginとは推測しない。
pub fn is_login_missing(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    text.contains("user is not authenticated to docker")
        || text.contains("you are not authenticated to docker")
        || text.contains("no valid user session found, please sign in to docker to proceed")
}
