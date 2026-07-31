pub(super) fn exec_arguments(sandbox: &str, user: Option<&str>, args: &[&str]) -> Vec<String> {
    let mut full: Vec<String> = vec!["exec".to_string()];
    if let Some(user) = user {
        full.push("--user".to_string());
        full.push(user.to_string());
    }
    full.push(sandbox.to_string());
    full.push("--".to_string());
    full.extend(args.iter().map(|arg| (*arg).to_string()));
    full
}
