/// `open`の観測がbare repositoryについて読むcommand。まとめた確認に含める。
///
/// 存在、bareかどうか、originのURL、fetch refspec、refの一覧、objectの数の順である。
/// 後ろの2つは、初期化の途中で止まったかどうかを`observe_initialization`が読む。
pub fn inspection_commands(git_dir: &str) -> Vec<Vec<String>> {
    let git: Vec<&[&str]> = vec![
        &["rev-parse", "--is-bare-repository"],
        &["config", "--get-all", "remote.origin.url"],
        &["config", "--get-all", "remote.origin.fetch"],
        &["for-each-ref", "--format=%(refname)"],
        &["count-objects", "-v"],
    ];
    let mut commands = vec![vec![
        "test".to_string(),
        "-e".to_string(),
        git_dir.to_string(),
    ]];
    for args in git {
        let mut command = vec![
            "git".to_string(),
            "--git-dir".to_string(),
            git_dir.to_string(),
        ];
        command.extend(args.iter().map(|arg| (*arg).to_string()));
        commands.push(command);
    }
    commands
}
