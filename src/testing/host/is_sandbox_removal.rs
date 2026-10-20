/// Sandbox削除の起動。secretの削除や、削除前のGETは含めない。
pub fn is_sandbox_removal(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "rm")
        || (args.iter().any(|arg| arg == "DELETE")
            && args.iter().any(|arg| {
                arg.starts_with("http://localhost/sandbox/")
                    && arg.ends_with("&delete_scoped_secrets=false")
            }))
}
