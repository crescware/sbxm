/// token環境変数fileを読む手順。`$1`がそのpathである。
///
/// dangling symlinkは存在する対象として扱い、rootでその行き先を作らない。通常fileも
/// symlinkも無い場合だけ、ほかの読み取り失敗と区別できる終了status（`44`）で答える。
pub(super) const TOKEN_ENV_READ: &str =
    r#"if [ ! -e "$1" ] && [ ! -L "$1" ]; then exit 44; fi; exec cat -- "$1""#;
