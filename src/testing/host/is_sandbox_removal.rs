/// Sandbox削除の起動。`sbx rm`と、daemonへのDELETEのどちらも含む。
///
/// 何も消さないことを確かめる側が使う。scoped secretを保持する指定の有無を問わず
/// 数える。secretの削除や、削除前のGETは含めない。
pub fn is_sandbox_removal(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "rm")
        || (args.iter().any(|arg| arg == "DELETE")
            && args
                .iter()
                .any(|arg| arg.starts_with("http://localhost/sandbox/")))
}
