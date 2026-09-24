use std::io::Write;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment, RealHost};
use crate::diagnostics::Result;

/// `sbx exec <sandbox> -- <argv>`を、Sandboxへ送らずこのhostでそのまま走らせるhost。
///
/// Sandboxの中で走る手順を、本物のgitとshellで確かめるために使う。`sbx exec`以外の
/// 起動もこのhostで走らせる。Sandboxの隔離は模さない。
pub struct LocalSandbox;

impl LocalSandbox {
    /// `sbx exec`の内側だけを取り出した起動。それ以外はそのまま返す。
    fn unwrap(spec: &CommandSpec) -> CommandSpec {
        let is_exec = spec.program == "sbx" && spec.args.first().is_some_and(|arg| arg == "exec");
        let Some(position) = spec.args.iter().position(|arg| arg == "--") else {
            return spec.clone();
        };
        if !is_exec {
            return spec.clone();
        }
        let inner: Vec<&str> = spec.args[position + 1..]
            .iter()
            .map(String::as_str)
            .collect();
        let Some((program, args)) = inner.split_first() else {
            return spec.clone();
        };
        let mut local = CommandSpec::capture(program, args).timeout(spec.timeout);
        if let Some(input) = spec.input() {
            local = local.with_input(input.to_vec());
        }
        if let Some(path) = &spec.input_file {
            local = local.with_input_file(path);
        }
        local
    }
}

impl HostEnvironment for LocalSandbox {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        RealHost.run(&Self::unwrap(spec))
    }

    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        RealHost.run_streaming(&Self::unwrap(spec), sink, limit)
    }
}
