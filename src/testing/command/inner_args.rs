use crate::command::CommandSpec;

/// `sbx exec <name> -- ...`の`--`より後ろ。`--`がなければ空。
pub fn inner_args(spec: &CommandSpec) -> Vec<&str> {
    spec.args
        .iter()
        .skip_while(|arg| *arg != "--")
        .skip(1)
        .map(String::as_str)
        .collect()
}
