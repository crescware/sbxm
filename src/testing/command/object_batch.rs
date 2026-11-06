use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::diagnostics::Result;

use super::outcome;

/// 既存のobject有無の台本を、Gitのbatch-checkと同じ順序・missing表記で答える。
pub fn object_batch(
    spec: &CommandSpec,
    mut answer: impl FnMut(&CommandSpec) -> Result<CommandOutcome>,
) -> Result<Option<CommandOutcome>> {
    if spec
        .args
        .last()
        .is_none_or(|arg| arg != "--batch-check=%(objectname)")
    {
        return Ok(None);
    }
    let mut output = String::new();
    for object in String::from_utf8_lossy(spec.input().unwrap_or_default()).lines() {
        let mut args = spec.args.clone();
        args.pop();
        args.push("-e".to_string());
        args.push(object.to_string());
        // -iはbatchのstdinのためだけにある。元の1件ずつの台本へは付けない。
        if args.get(1).is_some_and(|arg| arg == "-i") {
            args.remove(1);
        }
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let query = CommandSpec::capture(&spec.program, &borrowed);
        let result = answer(&query)?;
        match result.status.code() {
            Some(0) => {
                output.push_str(object);
                output.push('\n');
            }
            Some(1) => {
                output.push_str(object);
                output.push_str(" missing\n");
            }
            _ => return Ok(Some(outcome(spec, result.status.code().unwrap_or(128), ""))),
        }
    }
    Ok(Some(outcome(spec, 0, &output)))
}
