use std::io::Write;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment, RealHost};
use crate::diagnostics::Result;

/// `sbx exec <sandbox> -- <argv>`を、Sandboxへ送らずこのhostでそのまま走らせるhost。
///
/// Sandboxの中で走る手順を、本物のgitとshellで確かめるために使う。`sbx exec`以外の
/// 起動もこのhostで走らせる。hostのgitがssh越しにSandboxのrepositoryへ届くURL
/// （`ssh://<sandbox>.sbx/<path>`）は、このhostの`<path>`へ読み替える。Sandboxの隔離は
/// 模さない。
pub struct LocalSandbox;

impl LocalSandbox {
    /// `sbx exec`の内側だけを取り出した起動。gitの起動は、SandboxへのURLを読み替える。
    /// それ以外はそのまま返す。
    fn unwrap(spec: &CommandSpec) -> CommandSpec {
        if spec.program == "git" {
            let mut local = spec.clone();
            local.args = spec
                .args
                .iter()
                .map(|arg| local_path(arg).unwrap_or_else(|| arg.clone()))
                .collect();
            return local;
        }
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
        local
    }
}

/// `ssh://<sandbox>.sbx/<path>`が指す、このhostの`/<path>`。
fn local_path(arg: &str) -> Option<String> {
    let (host, path) = arg.strip_prefix("ssh://")?.split_once('/')?;
    let (_, domain) = host.rsplit_once('.')?;
    (domain == "sbx").then(|| format!("/{path}"))
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
