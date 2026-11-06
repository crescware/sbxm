/// 初期化前の失敗から再開できるdirectoryかを読み取る。既存の内容を消さない。
pub(super) fn unfinished_init_command(git_dir: &str) -> Vec<String> {
    [
        "sh",
        "-c",
        r#"[ ! -L "$1" ] && [ -d "$1" ] || exit 1
if [ -d "$1/sbxm-initializing" ] && [ ! -L "$1/sbxm-initializing" ]; then exit 0; fi
entries=$(ls -A -- "$1") || exit 2
[ -z "$entries" ]"#,
        "sh",
        git_dir,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}
