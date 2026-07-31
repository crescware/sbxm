pub fn argv(arguments: &[&str]) -> Vec<String> {
    std::iter::once("sbxm".to_string())
        .chain(arguments.iter().map(|value| (*value).to_string()))
        .collect()
}
