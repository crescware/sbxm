/// tokenの登録を解くcommand。
///
/// custom secretはenvでもhostでもなくplaceholderで指す。`sbx secret ls`のcustom secretの
/// 表にNAME列はなく、placeholderがこの登録を一意に示す唯一の公開値である。
///
/// `--placeholder`は`sbx secret rm --help`のFlagsに現れないhidden flagであり、
/// `同じhelpのexamplesとCLI` referenceが用法を示す。`--help`の一覧にないことを根拠に
/// 別の指定へ書き換えない。
///
/// scopeはSandbox名をpositionalで渡して確定させる。`-g`もSandbox名も渡さない実行は
/// scopeの選択を対話で訊く。sbxmはstdinを閉じて外部commandを実行するため、
/// 対話へ入った時点で答える手段がない。
///
/// `--force`は`sbx`の確認promptを省く。消してよいかはsbxmが先に判定しており、
/// `destroy`は自前の確認も済ませている。
pub fn forget_command(sandbox: &str, placeholder: &str) -> String {
    format!("sbx secret rm {sandbox} --placeholder {placeholder} --force")
}
