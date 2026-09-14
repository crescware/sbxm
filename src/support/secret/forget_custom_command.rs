/// 以前の版が案内したcustom secretの登録を解くcommand。
///
/// custom secretはenvでもhostでもなくplaceholderで指す。`sbx secret ls`のcustom
/// secretの表にNAME列はなく、placeholderがこの登録を一意に示す唯一の公開値である。
///
/// `--placeholder`は`sbx secret rm --help`のFlagsに現れないhidden flagであり、
/// `同じhelpのexamplesとCLI` referenceが用法を示す。`--help`の一覧にないことを根拠に
/// 別の指定へ書き換えない。
pub fn forget_custom_command(sandbox: &str, placeholder: &str) -> String {
    format!("sbx secret rm --sandbox {sandbox} --placeholder {placeholder} --force")
}
