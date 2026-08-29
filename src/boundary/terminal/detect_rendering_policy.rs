use console::Term;

use crate::design::policy::{Environment, Terminals};
use crate::design::{ColorSetting, RenderingPolicy};

/// processの環境変数とstreamを一度だけ観測し、純粋なdesign policyへ渡す。
pub(crate) fn detect_rendering_policy(setting: ColorSetting) -> RenderingPolicy {
    use std::io::IsTerminal;

    let terminal = Term::stderr();
    let environment = Environment {
        no_color: std::env::var_os("NO_COLOR").is_some(),
        clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
        term: std::env::var("TERM").ok(),
    };
    let terminals = Terminals {
        stdout_is_tty: std::io::stdout().is_terminal(),
        stderr_is_tty: std::io::stderr().is_terminal(),
        width: terminal.is_term().then(|| usize::from(terminal.size().1)),
    };
    RenderingPolicy::resolve(setting, &environment, &terminals)
}
